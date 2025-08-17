use crate::commands::highlighter::apply_syntax_highlight;

use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::response::Response;
use axum::{
    Router,
    extract::{Path, State},
    response::Html,
    routing::get,
};

use futures_util::{sink::SinkExt, stream::StreamExt};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::sync::broadcast;

// STEP 1 -> User should run the share command:

pub async fn start_sharing(file_path: PathBuf) -> Result<(), ()> {
    // Generate a unique id
    let share_id = uuid::Uuid::new_v4().to_string()[..8].to_string();

    // Read the file content
    let content = tokio::fs::read_to_string(&file_path).await.unwrap();

    // Creating broadcasting channel -> Capacity = 100
    let (tx, _rx) = broadcast::channel(100);

    let app_state = Arc::new(AppState {
        file_path: file_path.clone(),
        content: Mutex::new(content),
        broadcast_tx: tx.clone(),
        connected_clients: Mutex::new(0),
    });

    // Start file watcher in background
    let file_watcher_state = app_state.clone();
    tokio::spawn(async move {
        watch_file_changes(file_path, file_watcher_state).await;
    });

    // Create HTTP Routes
    let app: Router = Router::new()
        .route("/share/:id", get(serve_file_page))
        .route("/ws/:id", get(websocket_handler))
        .with_state(app_state);

    Ok(())
}

async fn serve_file_page(Path(share_id): Path<String>) -> Html<String> {
    // This serves an HTML page that automatically connects via WebSocket
    let html = format!(
        r#"
    <!DOCTYPE html>
    <html>
    <head><title>Shared File: {}</title></head>
    <body>
        <pre id="content">Loading...</pre>
        <script>
            const ws = new WebSocket('ws://localhost:3000/ws/{}');
            ws.onmessage = function(event) {{
                const data = JSON.parse(event.data);
                if (data.type === 'file_content') {{
                    document.getElementById('content').textContent = data.content;
                }}
            }};
        </script>
    </body>
    </html>
    "#,
        share_id, share_id
    );

    Html(html)
}

async fn websocket_handler(
    Path(share_id): Path<String>,
    ws: WebSocketUpgrade,
    State(state): State<Arc<AppState>>,
) -> Response {
    ws.on_upgrade(move |socket| handle_websocket(socket, state))
}

async fn handle_websocket(socket: WebSocket, state: Arc<AppState>) {
    let (mut sender, mut receiver) = socket.split();

    // Send initial file content
    let content = state.content.lock().await.clone();
    let initial_message = serde_json::json!({
        "type": "file_content",
        "content": content,
        "timestamp": chrono::Utc::now().to_rfc3339()
    });

    if sender
        .send(Message::Text(initial_message.to_string().into()))
        .await
        .is_err()
    {
        return; // Connection failed
    }

    // Subscribe to file updates
    let mut broadcast_rx = state.broadcast_tx.subscribe();

    // Increment connected clients
    *state.connected_clients.lock().await += 1;
    println!(
        "📱 New viewer connected! Total: {}",
        *state.connected_clients.lock().await
    );

    // Handle incoming updates and send to this client
    loop {
        tokio::select! {
            // Receive file updates from broadcast channel
            update = broadcast_rx.recv() => {
                match update {
                    Ok(file_content) => {
                        let message = serde_json::json!({
                            "type": "file_update",
                            "content": file_content,
                            "timestamp": chrono::Utc::now().to_rfc3339()
                        });


                        if sender.send(Message::Text(message.to_string().into())).await.is_err() {
                            break; // Connection lost
                        }
                    }
                    Err(_) => break, // Channel closed
                }
            }

            // Handle client messages (heartbeat, etc.)
            msg = receiver.next() => {
                match msg {
                    Some(Ok(Message::Close(_))) => break,
                    Some(Err(_)) => break,
                    None => break,
                    _ => {} // Ignore other messages
                }
            }
        }
    }

    // Client disconnected
    *state.connected_clients.lock().await -= 1;
    println!(
        "📱 Viewer disconnected. Remaining: {}",
        *state.connected_clients.lock().await
    );
}

async fn watch_file_changes(file_path: PathBuf, state: Arc<AppState>) {
    use notify::{Event, RecommendedWatcher, RecursiveMode, Watcher};
    use tokio::sync::mpsc;

    let (tx, mut rx) = mpsc::channel(100);

    let mut watcher: RecommendedWatcher = notify::Watcher::new(
        move |res: Result<Event, notify::Error>| {
            if let Ok(event) = res {
                let _ = tx.blocking_send(event);
            }
        },
        notify::Config::default(),
    )
    .expect("Failed to create watcher");

    watcher
        .watch(&file_path, RecursiveMode::NonRecursive)
        .expect("Failed to watch file");

    println!("👀 Watching file for changes: {:?}", file_path);

    while let Some(event) = rx.recv().await {
        if event.kind.is_modify() {
            // File was modified, read new content
            match tokio::fs::read_to_string(&file_path).await {
                Ok(new_content) => {
                    // Update stored content
                    *state.content.lock().await = new_content.clone();

                    // Broadcast to all connected clients
                    if let Err(_) = state.broadcast_tx.send(new_content) {
                        // No receivers, that's OK
                    }

                    println!(
                        "📝 File updated, broadcasting to {} clients",
                        *state.connected_clients.lock().await
                    );
                }
                Err(e) => {
                    eprintln!("Error reading file: {}", e);
                }
            }
        }
    }
}

struct AppState {
    file_path: PathBuf,
    content: Mutex<String>,
    broadcast_tx: broadcast::Sender<String>,
    connected_clients: Mutex<usize>,
}

pub async fn connect_to_share(url: &str) -> Result<(), ()> {
    // Parse URL to get WebSocket endpoint
    let ws_url = url.replace("/share/", "/ws/").replace("http://", "ws://");

    // Connect to WebSocket
    let (ws_stream, _) = tokio_tungstenite::connect_async(&ws_url).await.unwrap();
    let (write, mut read) = ws_stream.split();

    println!("🔗 Connected to shared file: {}", url);
    println!("📡 Receiving live updates...\n");

    // Listen for updates
    while let Some(msg) = read.next().await {
        match msg {
            Ok(tokio_tungstenite::tungstenite::Message::Text(text)) => {
                let data: serde_json::Value = serde_json::from_str(&text).unwrap();

                if data["type"] == "file_content" || data["type"] == "file_update" {
                    let content = data["content"].as_str().unwrap_or("");

                    // Clear screen and show updated content
                    print!("\x1b[2J\x1b[H"); // Clear screen, move cursor to top

                    // Apply syntax highlighting to the content
                    apply_syntax_highlight(&content, ".rs");

                    println!("\n--- Live updates enabled (Ctrl+C to exit) ---");
                }
            }
            Ok(tokio_tungstenite::tungstenite::Message::Close(..)) => {
                println!("Connection closed");
                break;
            }
            _ => {}
        }
    }

    Ok(())
}
