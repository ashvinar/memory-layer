// Memory Layer Popup Script

document.addEventListener('DOMContentLoaded', init);

async function init() {
  // Check health status
  checkHealth();

  // Set up button listeners
  document.getElementById('test-btn')?.addEventListener('click', testContext);
  document.getElementById('open-settings')?.addEventListener('click', openSettings);

  // Check health every 2 seconds while popup is open
  setInterval(checkHealth, 2000);
}

async function checkHealth() {
  const response = await chrome.runtime.sendMessage({
    action: 'checkHealth'
  });

  const statusEl = document.getElementById('status');
  const composerStatus = document.getElementById('composer-status');
  const indexingStatus = document.getElementById('indexing-status');
  const ingestionStatus = document.getElementById('ingestion-status');

  if (response?.healthy) {
    statusEl.className = 'status healthy';
    statusEl.textContent = 'All services running';

    composerStatus.className = 'service-status online';
    indexingStatus.className = 'service-status online';
    ingestionStatus.className = 'service-status online';

    document.getElementById('test-btn').disabled = false;
  } else {
    statusEl.className = 'status unhealthy';
    statusEl.textContent = 'Services offline - Start the backend first';

    composerStatus.className = 'service-status offline';
    indexingStatus.className = 'service-status offline';
    ingestionStatus.className = 'service-status offline';

    document.getElementById('test-btn').disabled = true;
  }

  // Show last check time
  if (response?.lastCheck) {
    const secondsAgo = Math.floor((Date.now() - response.lastCheck) / 1000);
    const timeText = secondsAgo < 5 ? 'just now' : `${secondsAgo}s ago`;
    statusEl.title = `Last checked: ${timeText}`;
  }
}

async function testContext() {
  const btn = document.getElementById('test-btn');
  const originalText = btn.textContent;
  btn.textContent = 'Testing...';
  btn.disabled = true;

  try {
    // Request a test context
    const response = await chrome.runtime.sendMessage({
      action: 'getContext',
      data: {
        topicHint: 'Extension Test',
        budgetTokens: 150,
        scopes: ['assistant']
      }
    });

    if (response.success) {
      const capsule = response.capsule;

      // Show success message
      btn.textContent = '✓ Success!';
      btn.style.background = '#10b981';

      // Create a simple alert with the result
      alert(`Context Generated Successfully!\n\nTokens: ${capsule.token_count}\nStyle: ${capsule.style}\n\nPreview:\n${capsule.preamble_text.substring(0, 200)}...`);

    } else {
      throw new Error(response.error || 'Unknown error');
    }

  } catch (error) {
    btn.textContent = '✗ Failed';
    btn.style.background = '#ef4444';
    alert(`Test failed: ${error.message}`);

  } finally {
    // Reset button after 2 seconds
    setTimeout(() => {
      btn.textContent = originalText;
      btn.style.background = '';
      btn.disabled = false;
    }, 2000);
  }
}

function openSettings() {
  // Open the extension options page
  chrome.runtime.openOptionsPage();
}