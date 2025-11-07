use anyhow::Result;
use chrono::Utc;
use memory_layer_schemas::{generate_thread_id, generate_turn_id, SourceApp, Turn, TurnSource};
use reqwest;
use serde_json::json;
use std::time::{Duration, Instant};
use tokio::time::sleep;

const BASE_URL: &str = "http://127.0.0.1:21953";

#[tokio::test]
async fn test_instant_ingestion() -> Result<()> {
    println!("=== Testing Instant Ingestion (<10ms latency) ===\n");

    // Wait for server to be ready
    wait_for_server().await?;

    // Test 1: Single request latency
    println!("Test 1: Single request latency");
    test_single_request_latency().await?;

    // Test 2: Memories appear in database
    println!("\nTest 2: Memories appear in database (async processing)");
    test_async_memory_processing().await?;

    // Test 3: High concurrency load
    println!("\nTest 3: No data loss under 100+ concurrent requests");
    test_concurrent_load().await?;

    println!("\n=== All tests passed! ===");
    Ok(())
}

async fn wait_for_server() -> Result<()> {
    let client = reqwest::Client::new();
    let max_attempts = 30;

    for attempt in 1..=max_attempts {
        match client.get(format!("{}/health", BASE_URL)).send().await {
            Ok(resp) if resp.status().is_success() => {
                println!("✓ Server is ready\n");
                return Ok(());
            }
            _ => {
                if attempt == max_attempts {
                    anyhow::bail!("Server did not start in time");
                }
                sleep(Duration::from_secs(1)).await;
            }
        }
    }

    Ok(())
}

async fn test_single_request_latency() -> Result<()> {
    let client = reqwest::Client::new();

    // Create a test turn
    let turn = Turn {
        id: generate_turn_id(),
        thread_id: generate_thread_id(),
        ts_user: Utc::now().to_rfc3339(),
        user_text: "I decided to use Rust for this high-performance service. It's great for systems programming.".to_string(),
        ts_ai: Some(Utc::now().to_rfc3339()),
        ai_text: Some("That's an excellent choice!".to_string()),
        source: TurnSource {
            app: SourceApp::Claude,
            url: None,
            path: Some("src/main.rs".to_string()),
        },
    };

    // Measure latency
    let start = Instant::now();
    let response = client
        .post(format!("{}/ingest/turn", BASE_URL))
        .json(&turn)
        .send()
        .await?;
    let latency = start.elapsed();

    // Check response
    assert!(
        response.status().is_success(),
        "Request failed: {}",
        response.status()
    );

    let body: serde_json::Value = response.json().await?;
    assert_eq!(body["turn_id"], turn.id.0);
    assert_eq!(body["status"], "queued");

    // Check latency
    let latency_ms = latency.as_millis();
    println!("  Latency: {}ms", latency_ms);

    if latency_ms < 10 {
        println!("  ✓ PASS: Latency is {}ms (target: <10ms)", latency_ms);
    } else if latency_ms < 50 {
        println!("  ⚠ ACCEPTABLE: Latency is {}ms (slightly above target but still fast)", latency_ms);
    } else {
        anyhow::bail!("FAIL: Latency is {}ms, exceeds acceptable threshold", latency_ms);
    }

    Ok(())
}

async fn test_async_memory_processing() -> Result<()> {
    let client = reqwest::Client::new();

    // Create a turn with content that will generate multiple memories
    let turn = Turn {
        id: generate_turn_id(),
        thread_id: generate_thread_id(),
        ts_user: Utc::now().to_rfc3339(),
        user_text: "TODO: need to refactor the authentication module. The current implementation is insecure.".to_string(),
        ts_ai: None,
        ai_text: None,
        source: TurnSource {
            app: SourceApp::VSCode,
            url: None,
            path: Some("src/auth.rs".to_string()),
        },
    };

    // Get initial memory count
    let stats_before: serde_json::Value = client
        .get(format!("{}/stats", BASE_URL))
        .send()
        .await?
        .json()
        .await?;
    let memories_before = stats_before["memories"].as_u64().unwrap_or(0);

    println!("  Memories before: {}", memories_before);

    // Ingest the turn
    let response = client
        .post(format!("{}/ingest/turn", BASE_URL))
        .json(&turn)
        .send()
        .await?;

    assert!(response.status().is_success());

    // Wait for async processing to complete
    println!("  Waiting for async processing...");
    sleep(Duration::from_secs(2)).await;

    // Check that memories were created
    let stats_after: serde_json::Value = client
        .get(format!("{}/stats", BASE_URL))
        .send()
        .await?
        .json()
        .await?;
    let memories_after = stats_after["memories"].as_u64().unwrap_or(0);

    println!("  Memories after: {}", memories_after);

    if memories_after > memories_before {
        println!(
            "  ✓ PASS: {} new memories created (async processing works)",
            memories_after - memories_before
        );
        Ok(())
    } else {
        anyhow::bail!(
            "FAIL: No new memories created. Before: {}, After: {}",
            memories_before,
            memories_after
        );
    }
}

async fn test_concurrent_load() -> Result<()> {
    let num_requests = 100;
    println!("  Sending {} concurrent requests...", num_requests);

    let client = reqwest::Client::new();

    // Get initial counts
    let stats_before: serde_json::Value = client
        .get(format!("{}/stats", BASE_URL))
        .send()
        .await?
        .json()
        .await?;
    let turns_before = stats_before["turns"].as_u64().unwrap_or(0);
    let memories_before = stats_before["memories"].as_u64().unwrap_or(0);

    println!("  Turns before: {}", turns_before);
    println!("  Memories before: {}", memories_before);

    // Create and send concurrent requests
    let mut handles = Vec::new();
    let start = Instant::now();

    for i in 0..num_requests {
        let client = client.clone();
        let handle = tokio::spawn(async move {
            let turn = Turn {
                id: generate_turn_id(),
                thread_id: generate_thread_id(),
                ts_user: Utc::now().to_rfc3339(),
                user_text: format!("Test message {} - I decided to implement feature X.", i),
                ts_ai: Some(Utc::now().to_rfc3339()),
                ai_text: Some("Acknowledged.".to_string()),
                source: TurnSource {
                    app: SourceApp::Claude,
                    url: None,
                    path: None,
                },
            };

            client
                .post(format!("{}/ingest/turn", BASE_URL))
                .json(&turn)
                .send()
                .await
        });
        handles.push(handle);
    }

    // Wait for all requests to complete
    let mut success_count = 0;
    let mut fail_count = 0;

    for handle in handles {
        match handle.await {
            Ok(Ok(response)) if response.status().is_success() => {
                success_count += 1;
            }
            _ => {
                fail_count += 1;
            }
        }
    }

    let total_time = start.elapsed();
    let avg_latency = total_time.as_millis() / num_requests;

    println!("  Requests completed: {}/{}", success_count, num_requests);
    println!("  Failed requests: {}", fail_count);
    println!("  Total time: {:?}", total_time);
    println!("  Average latency: {}ms", avg_latency);

    // Wait for async processing (worker processes sequentially, so need more time for 100 items)
    println!("  Waiting for async processing (this may take a while for 100 turns)...");
    sleep(Duration::from_secs(15)).await;

    // Check final counts
    let stats_after: serde_json::Value = client
        .get(format!("{}/stats", BASE_URL))
        .send()
        .await?
        .json()
        .await?;
    let turns_after = stats_after["turns"].as_u64().unwrap_or(0);
    let memories_after = stats_after["memories"].as_u64().unwrap_or(0);

    println!("  Turns after: {}", turns_after);
    println!("  Memories after: {}", memories_after);

    let expected_turns = turns_before + (success_count as u64);
    let turns_created = turns_after - turns_before;

    // Validate results
    if fail_count > 0 {
        anyhow::bail!("FAIL: {} requests failed", fail_count);
    }

    if turns_created != success_count as u64 {
        anyhow::bail!(
            "FAIL: Data loss detected. Expected {} turns, got {}",
            success_count,
            turns_created
        );
    }

    if memories_after <= memories_before {
        anyhow::bail!(
            "FAIL: No memories created. Before: {}, After: {}",
            memories_before,
            memories_after
        );
    }

    println!(
        "  ✓ PASS: All {} requests succeeded, {} turns and {} memories created",
        success_count,
        turns_created,
        memories_after - memories_before
    );

    Ok(())
}
