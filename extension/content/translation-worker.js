// Translation Web Worker for Moon Translator
// Handles translation requests off the main thread

// Worker state
let pendingRequests = new Map();
let requestIdCounter = 0;

// Message handler
self.onmessage = function(e) {
  const { type, id, payload } = e.data;

  switch (type) {
    case "translate":
      handleTranslate(id, payload);
      break;
    case "batchTranslate":
      handleBatchTranslate(id, payload);
      break;
    case "cancel":
      handleCancel(id);
      break;
  }
};

// Handle single translation request
async function handleTranslate(id, payload) {
  const { text, from, to } = payload;

  try {
    // Send request to background via postMessage
    // Worker cannot access chrome.runtime directly
    self.postMessage({
      type: "requestTranslation",
      id,
      payload: { text, from, to }
    });

    // Store pending request
    pendingRequests.set(id, { resolve: null, reject: null });
  } catch (error) {
    self.postMessage({
      type: "translationError",
      id,
      error: error.message
    });
  }
}

// Handle batch translation request
async function handleBatchTranslate(id, payload) {
  const { texts, from, to, batchSize = 3 } = payload;

  try {
    // Split into batches
    const batches = [];
    for (let i = 0; i < texts.length; i += batchSize) {
      batches.push({
        batchIndex: Math.floor(i / batchSize),
        texts: texts.slice(i, i + batchSize),
        startIndex: i
      });
    }

    // Process batches sequentially to avoid overwhelming the API
    const results = new Array(texts.length);
    let processedCount = 0;

    for (const batch of batches) {
      // Send batch to background
      self.postMessage({
        type: "requestBatchTranslation",
        id: `${id}_batch_${batch.batchIndex}`,
        parentId: id,
        payload: {
          texts: batch.texts,
          from,
          to,
          startIndex: batch.startIndex
        }
      });

      // Wait for batch response
      const batchResult = await waitForResponse(`${id}_batch_${batch.batchIndex}`);

      if (batchResult.success) {
        batchResult.results.forEach((result, idx) => {
          results[batch.startIndex + idx] = result;
        });
      }

      processedCount += batch.texts.length;

      // Report progress
      self.postMessage({
        type: "progress",
        id,
        processed: processedCount,
        total: texts.length
      });
    }

    // Send final result
    self.postMessage({
      type: "batchTranslationComplete",
      id,
      results
    });
  } catch (error) {
    self.postMessage({
      type: "translationError",
      id,
      error: error.message
    });
  }
}

// Handle cancellation
function handleCancel(id) {
  pendingRequests.delete(id);
}

// Wait for response from main thread
function waitForResponse(requestId) {
  return new Promise((resolve, reject) => {
    pendingRequests.set(requestId, { resolve, reject });

    // Timeout after 30 seconds
    setTimeout(() => {
      if (pendingRequests.has(requestId)) {
        pendingRequests.delete(requestId);
        reject(new Error("Translation request timeout"));
      }
    }, 30000);
  });
}

// Receive response from main thread
self.onmessage = function(e) {
  const { type, id, payload } = e.data;

  // Handle responses to pending requests
  if (type === "translationResponse" && pendingRequests.has(id)) {
    const { resolve } = pendingRequests.get(id);
    pendingRequests.delete(id);
    if (resolve) resolve(payload);
    return;
  }

  if (type === "translationErrorResponse" && pendingRequests.has(id)) {
    const { reject } = pendingRequests.get(id);
    pendingRequests.delete(id);
    if (reject) reject(new Error(payload.error));
    return;
  }

  // Handle new requests
  switch (type) {
    case "translate":
      handleTranslate(id, payload);
      break;
    case "batchTranslate":
      handleBatchTranslate(id, payload);
      break;
    case "cancel":
      handleCancel(id);
      break;
  }
};

// Signal that worker is ready
self.postMessage({ type: "ready" });
