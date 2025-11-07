// Memory Layer Chrome Extension - Service Worker (MV3)

const COMPOSER_API_URL = 'http://127.0.0.1:21955';
const INDEXING_API_URL = 'http://127.0.0.1:21954';
const INGESTION_API_URL = 'http://127.0.0.1:21953';

// Service state
let isHealthy = false;
let lastHealthCheck = 0;

// Initialize extension
chrome.runtime.onInstalled.addListener(() => {
  console.log('Memory Layer Extension installed');
  checkServiceHealth();

  // Set up periodic health checks
  setInterval(checkServiceHealth, 30000); // Every 30 seconds
});

// Check if backend services are running
async function checkServiceHealth() {
  try {
    const response = await fetch(`${COMPOSER_API_URL}/health`);
    isHealthy = response.ok;
    lastHealthCheck = Date.now();

    // Update badge based on health
    chrome.action.setBadgeText({
      text: isHealthy ? '' : '!'
    });
    chrome.action.setBadgeBackgroundColor({
      color: isHealthy ? '#00AA00' : '#FF0000'
    });
  } catch (error) {
    isHealthy = false;
    console.error('Health check failed:', error);
  }
}

// Handle messages from content scripts
chrome.runtime.onMessage.addListener((request, sender, sendResponse) => {
  console.log('Received message:', request.action);

  switch (request.action) {
    case 'getContext':
      handleGetContext(request.data).then(sendResponse);
      return true; // Will respond asynchronously

    case 'undoContext':
      handleUndoContext(request.data).then(sendResponse);
      return true;

    case 'checkHealth':
      sendResponse({ healthy: isHealthy, lastCheck: lastHealthCheck });
      return false;

    case 'search':
      handleSearch(request.data).then(sendResponse);
      return true;

    case 'ingestTurn':
      handleIngestTurn(request.data).then(sendResponse);
      return true;

    default:
      sendResponse({ error: 'Unknown action' });
      return false;
  }
});

// Get context from composer service
async function handleGetContext(data) {
  try {
    const response = await fetch(`${COMPOSER_API_URL}/v1/context`, {
      method: 'POST',
      headers: {
        'Content-Type': 'application/json'
      },
      body: JSON.stringify({
        topic_hint: data.topicHint || null,
        intent: data.intent || null,
        budget_tokens: data.budgetTokens || 220,
        scopes: data.scopes || ['assistant'],
        thread_key: data.threadKey || null,
        last_capsule_id: data.lastCapsuleId || null
      })
    });

    if (!response.ok) {
      throw new Error(`HTTP error! status: ${response.status}`);
    }

    const capsule = await response.json();
    return { success: true, capsule };

  } catch (error) {
    console.error('Failed to get context:', error);
    return { success: false, error: error.message };
  }
}

// Undo context injection
async function handleUndoContext(data) {
  try {
    const response = await fetch(`${COMPOSER_API_URL}/v1/undo`, {
      method: 'POST',
      headers: {
        'Content-Type': 'application/json'
      },
      body: JSON.stringify({
        capsule_id: data.capsuleId,
        thread_key: data.threadKey
      })
    });

    const result = await response.json();
    return { success: result.success };

  } catch (error) {
    console.error('Failed to undo context:', error);
    return { success: false, error: error.message };
  }
}

// Search memories
async function handleSearch(data) {
  try {
    const params = new URLSearchParams({
      q: data.query,
      limit: data.limit || '10'
    });

    const response = await fetch(`${INDEXING_API_URL}/search?${params}`);

    if (!response.ok) {
      throw new Error(`HTTP error! status: ${response.status}`);
    }

    const results = await response.json();
    return { success: true, results };

  } catch (error) {
    console.error('Search failed:', error);
    return { success: false, error: error.message };
  }
}

// Ingest a turn (text from page)
async function handleIngestTurn(data) {
  try {
    const response = await fetch(`${INGESTION_API_URL}/ingest/turn`, {
      method: 'POST',
      headers: {
        'Content-Type': 'application/json'
      },
      body: JSON.stringify({
        id: generateTurnId(),
        thread_id: data.threadId || 'thr_chrome_ext',
        role: data.role || 'user',
        text: data.text,
        timestamp: new Date().toISOString(),
        source: 'chrome-ext',
        context: {
          url: data.url,
          title: data.title
        }
      })
    });

    const result = await response.json();
    return { success: true, turnId: result.id };

  } catch (error) {
    console.error('Failed to ingest turn:', error);
    return { success: false, error: error.message };
  }
}

// Helper to generate turn IDs
function generateTurnId() {
  const timestamp = Date.now().toString(36);
  const random = Math.random().toString(36).substr(2, 9);
  return `tur_${timestamp}${random}`;
}

// Listen for tab updates to detect AI assistant pages
chrome.tabs.onUpdated.addListener((tabId, changeInfo, tab) => {
  if (changeInfo.status === 'complete') {
    const url = tab.url || '';

    // Check if this is an AI assistant page we support
    if (url.includes('claude.ai') || url.includes('chatgpt.com') || url.includes('chat.openai.com')) {
      // Notify content script that page is ready
      chrome.tabs.sendMessage(tabId, {
        action: 'pageReady',
        url: url
      });
    }
  }
});

console.log('Memory Layer Service Worker loaded');