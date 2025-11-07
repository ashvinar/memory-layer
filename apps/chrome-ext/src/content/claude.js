// Memory Layer - Claude.ai Content Script

(() => {
  console.log('Memory Layer: Claude content script loaded');

  let currentCapsuleId = null;
  let currentThreadKey = 'claude_' + Date.now();
  let isContextApplied = false;

  // Initialize when page is ready
  if (document.readyState === 'loading') {
    document.addEventListener('DOMContentLoaded', init);
  } else {
    init();
  }

  function init() {
    console.log('Memory Layer: Initializing Claude integration');

    // Add context pill to input area
    addContextPill();

    // Monitor for new conversations
    observeConversationChanges();

    // Listen for messages from background script
    chrome.runtime.onMessage.addListener(handleMessage);
  }

  // Add "Add Context" pill to the input area
  function addContextPill() {
    const inputArea = findInputArea();
    if (!inputArea) {
      console.log('Input area not found, retrying...');
      setTimeout(addContextPill, 1000);
      return;
    }

    // Check if pill already exists
    if (document.querySelector('.memory-layer-pill')) {
      return;
    }

    // Create the pill container
    const pillContainer = document.createElement('div');
    pillContainer.className = 'memory-layer-pill-container';
    pillContainer.innerHTML = `
      <button class="memory-layer-pill" title="Add context from Memory Layer">
        <svg width="16" height="16" viewBox="0 0 16 16" fill="none">
          <path d="M8 2L10 4L8 6L6 4L8 2Z" fill="currentColor"/>
          <path d="M8 10L10 12L8 14L6 12L8 10Z" fill="currentColor"/>
          <path d="M2 8L4 6L6 8L4 10L2 8Z" fill="currentColor"/>
          <path d="M10 8L12 6L14 8L12 10L10 8Z" fill="currentColor"/>
        </svg>
        Add Context
      </button>
    `;

    // Style the pill
    const style = document.createElement('style');
    style.textContent = `
      .memory-layer-pill-container {
        position: absolute;
        top: 10px;
        right: 10px;
        z-index: 1000;
      }

      .memory-layer-pill {
        display: flex;
        align-items: center;
        gap: 6px;
        padding: 6px 12px;
        background: rgba(59, 130, 246, 0.1);
        border: 1px solid rgba(59, 130, 246, 0.3);
        border-radius: 16px;
        color: #3b82f6;
        font-size: 12px;
        font-weight: 500;
        cursor: pointer;
        transition: all 0.2s;
      }

      .memory-layer-pill:hover {
        background: rgba(59, 130, 246, 0.2);
        border-color: rgba(59, 130, 246, 0.5);
      }

      .memory-layer-ribbon {
        position: fixed;
        top: 60px;
        right: 20px;
        padding: 8px 16px;
        background: #10b981;
        color: white;
        border-radius: 8px;
        font-size: 13px;
        font-weight: 500;
        z-index: 10000;
        display: flex;
        align-items: center;
        gap: 8px;
        box-shadow: 0 4px 6px rgba(0, 0, 0, 0.1);
        animation: slideIn 0.3s ease-out;
      }

      @keyframes slideIn {
        from {
          transform: translateX(120%);
          opacity: 0;
        }
        to {
          transform: translateX(0);
          opacity: 1;
        }
      }

      .memory-layer-undo-btn {
        background: rgba(255, 255, 255, 0.2);
        border: 1px solid rgba(255, 255, 255, 0.3);
        color: white;
        padding: 2px 8px;
        border-radius: 4px;
        font-size: 12px;
        cursor: pointer;
        transition: all 0.2s;
      }

      .memory-layer-undo-btn:hover {
        background: rgba(255, 255, 255, 0.3);
      }
    `;
    document.head.appendChild(style);

    // Position relative to input area
    const inputParent = inputArea.parentElement;
    inputParent.style.position = 'relative';
    inputParent.appendChild(pillContainer);

    // Handle click on pill
    pillContainer.querySelector('.memory-layer-pill').addEventListener('click', handleAddContext);
  }

  // Find the Claude input area
  function findInputArea() {
    // Try different selectors for Claude's input
    const selectors = [
      '[contenteditable="true"]',
      'textarea',
      '.ProseMirror',
      '[role="textbox"]'
    ];

    for (const selector of selectors) {
      const element = document.querySelector(selector);
      if (element) {
        return element;
      }
    }

    return null;
  }

  // Handle adding context
  async function handleAddContext() {
    console.log('Memory Layer: Requesting context...');

    // Get current page context
    const pageContext = extractPageContext();

    // Request context from background script
    const response = await chrome.runtime.sendMessage({
      action: 'getContext',
      data: {
        topicHint: pageContext.topic,
        intent: pageContext.lastMessage,
        budgetTokens: 220,
        scopes: ['assistant'],
        threadKey: currentThreadKey,
        lastCapsuleId: currentCapsuleId
      }
    });

    if (response.success) {
      const capsule = response.capsule;
      currentCapsuleId = capsule.capsule_id;

      // Insert context into input
      insertContext(capsule.preamble_text);

      // Show success ribbon
      showRibbon('Context applied', capsule.token_count);

      isContextApplied = true;
    } else {
      console.error('Failed to get context:', response.error);
      showRibbon('Failed to get context', 0, true);
    }
  }

  // Extract context from the current page
  function extractPageContext() {
    // Get the last user message if available
    const messages = document.querySelectorAll('[data-testid="user-message"], .user-message');
    const lastMessage = messages.length > 0 ?
      messages[messages.length - 1].textContent?.slice(0, 500) : '';

    // Try to extract topic from conversation title or first message
    const title = document.querySelector('h1, [data-testid="conversation-title"]')?.textContent || '';

    return {
      topic: title || 'Claude Conversation',
      lastMessage: lastMessage
    };
  }

  // Insert context into the input field
  function insertContext(preambleText) {
    const inputArea = findInputArea();
    if (!inputArea) {
      console.error('Could not find input area');
      return;
    }

    // Add context as a prefill
    const contextBlock = `[Context from Memory Layer]\n${preambleText}\n\n`;

    // Handle different input types
    if (inputArea.tagName === 'TEXTAREA') {
      inputArea.value = contextBlock + inputArea.value;
      inputArea.dispatchEvent(new Event('input', { bubbles: true }));
    } else if (inputArea.contentEditable === 'true') {
      // For contenteditable (ProseMirror)
      const currentContent = inputArea.innerText || '';
      inputArea.innerText = contextBlock + currentContent;

      // Trigger input event
      inputArea.dispatchEvent(new Event('input', { bubbles: true }));

      // Move cursor to end
      const range = document.createRange();
      const sel = window.getSelection();
      range.selectNodeContents(inputArea);
      range.collapse(false);
      sel?.removeAllRanges();
      sel?.addRange(range);
    }
  }

  // Show success/error ribbon
  function showRibbon(message, tokenCount, isError = false) {
    // Remove existing ribbon if any
    const existingRibbon = document.querySelector('.memory-layer-ribbon');
    if (existingRibbon) {
      existingRibbon.remove();
    }

    const ribbon = document.createElement('div');
    ribbon.className = 'memory-layer-ribbon';
    ribbon.style.background = isError ? '#ef4444' : '#10b981';

    const tokensText = tokenCount ? ` (${tokenCount} tokens)` : '';
    ribbon.innerHTML = `
      <span>${message}${tokensText}</span>
      ${!isError ? '<button class="memory-layer-undo-btn">Undo</button>' : ''}
    `;

    document.body.appendChild(ribbon);

    // Handle undo
    if (!isError) {
      ribbon.querySelector('.memory-layer-undo-btn')?.addEventListener('click', handleUndo);
    }

    // Auto-hide after 5 seconds
    setTimeout(() => {
      ribbon.style.animation = 'slideIn 0.3s ease-out reverse';
      setTimeout(() => ribbon.remove(), 300);
    }, 5000);
  }

  // Handle undo
  async function handleUndo() {
    if (!currentCapsuleId) return;

    const response = await chrome.runtime.sendMessage({
      action: 'undoContext',
      data: {
        capsuleId: currentCapsuleId,
        threadKey: currentThreadKey
      }
    });

    if (response.success) {
      // Remove context from input
      const inputArea = findInputArea();
      if (inputArea) {
        const content = inputArea.innerText || inputArea.value || '';
        const cleanContent = content.replace(/\[Context from Memory Layer\][\s\S]*?\n\n/, '');

        if (inputArea.tagName === 'TEXTAREA') {
          inputArea.value = cleanContent;
        } else {
          inputArea.innerText = cleanContent;
        }
      }

      currentCapsuleId = null;
      isContextApplied = false;
      showRibbon('Context removed', 0);
    }
  }

  // Observe for conversation changes
  function observeConversationChanges() {
    const observer = new MutationObserver((mutations) => {
      // Check if URL changed (new conversation)
      if (window.location.href !== observer.lastUrl) {
        observer.lastUrl = window.location.href;
        currentThreadKey = 'claude_' + Date.now();
        currentCapsuleId = null;
        isContextApplied = false;

        // Re-add context pill
        setTimeout(addContextPill, 500);
      }
    });

    observer.lastUrl = window.location.href;
    observer.observe(document.body, {
      childList: true,
      subtree: true
    });
  }

  // Handle messages from background script
  function handleMessage(request, sender, sendResponse) {
    if (request.action === 'pageReady') {
      // Re-initialize when page is ready
      init();
    }
  }
})();