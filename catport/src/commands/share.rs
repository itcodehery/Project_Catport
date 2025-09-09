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

pub async fn start_sharing(file_path: PathBuf) -> Result<(), Box<dyn std::error::Error>> {
    // Generate a unique id
    let share_id = uuid::Uuid::new_v4().to_string()[..8].to_string();
    println!("🚀 Starting server for file: {:?}", file_path);

    // Read the file content
    let content = tokio::fs::read_to_string(&file_path)
        .await
        .map_err(|e| format!("Failed to read file: {}", e))?;

    // Creating broadcasting channel -> Capacity = 100
    let (tx, _rx) = broadcast::channel(100);

    let app_state = Arc::new(AppState {
        share_id: share_id.clone(),
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
        .route("/share/{id}", get(serve_file_page))
        .route("/ws/{id}", get(websocket_handler))
        .with_state(app_state);

    // ✅ Bind to 0.0.0.0 to accept connections from any network interface
    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await?;

    // Get and display the local IP address
    let local_ip = get_local_ip().unwrap_or_else(|| "localhost".to_string());

    println!("🎯 Server listening on:");
    println!("  📱 Network: http://{}:3000/share/{}", local_ip, share_id);
    println!("  💻 Local:   http://localhost:3000/share/{}", share_id);
    println!("  📡 Use the Network URL on your phone!");

    axum::serve(listener, app).await?;

    Ok(())
}

// Helper function to get the local IP address
fn get_local_ip() -> Option<String> {
    use std::net::{IpAddr, Ipv4Addr};

    // Try to get local IP by connecting to a remote address
    let socket = std::net::UdpSocket::bind("0.0.0.0:0").ok()?;
    socket.connect("8.8.8.8:80").ok()?; // Connect to Google DNS
    let local_addr = socket.local_addr().ok()?;

    match local_addr.ip() {
        IpAddr::V4(ip) => Some(ip.to_string()),
        _ => None,
    }
}

async fn serve_file_page(
    Path(requested_id): Path<String>,
    State(state): State<Arc<AppState>>,
) -> Result<Html<String>, axum::http::StatusCode> {
    // Validate the share ID
    if requested_id != state.share_id {
        return Err(axum::http::StatusCode::NOT_FOUND);
    }

    let html = format!(
        r#"
    <!DOCTYPE html>
    <html>
    <head>
        <title>📁 Catport Live Share</title>
        <meta name="viewport" content="width=device-width, initial-scale=1">
        <style>
            body {{ 
                font-family: 'Courier New', monospace; 
                margin: 20px; 
                background: #1e1e1e;
                color: #d4d4d4;
            }}
            h1 {{ color: #569cd6; }}
            #content {{ 
                background: #2d2d30; 
                padding: 20px; 
                border-radius: 8px;
                white-space: pre-wrap;
                border: 1px solid #3e3e42;
                font-size: 14px;
                line-height: 1.4;
                overflow-x: auto;
            }}
            .status {{
                padding: 10px;
                border-radius: 5px;
                margin: 10px 0;
            }}
            .connected {{ background: #1e3a1e; color: #4ec94e; }}
            .disconnected {{ background: #3a1e1e; color: #f44747; }}
        </style>
    </head>
    <body>
        <h1>🐾 Catport Live Share</h1>
        <div id="status" class="status disconnected">🔌 Connecting...</div>
        <pre id="content">Loading...</pre>
        
        <script>
            // ✅ Use dynamic WebSocket URL based on current host
            const protocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:';
            const wsUrl = `${{protocol}}//${{window.location.host}}/ws/{}`;
            
            console.log('Connecting to:', wsUrl);
            const ws = new WebSocket(wsUrl);
            
            const statusEl = document.getElementById('status');
            const contentEl = document.getElementById('content');
            
            ws.onopen = function() {{
                console.log('✅ Connected to live share');
                statusEl.textContent = '🟢 Connected - Live updates enabled';
                statusEl.className = 'status connected';
            }};
            
            ws.onmessage = function(event) {{
                try {{
                    const data = JSON.parse(event.data);
                    if (data.type === 'file_content' || data.type === 'file_update') {{
                        contentEl.textContent = data.content;
                        if (data.type === 'file_update') {{
                            // Brief highlight effect on updates
                            contentEl.style.background = '#3a3a1e';
                            setTimeout(() => {{
                                contentEl.style.background = '#2d2d30';
                            }}, 200);
                        }}
                    }}
                }} catch (e) {{
                    console.error('Error parsing message:', e);
                }}
            }};
            
            ws.onerror = function(error) {{
                console.error('❌ WebSocket error:', error);
                statusEl.textContent = '🔴 Connection error';
                statusEl.className = 'status disconnected';
            }};
            
            ws.onclose = function() {{
                console.log('🔌 Connection closed');
                statusEl.textContent = '🔴 Disconnected';
                statusEl.className = 'status disconnected';
            }};
        </script>
    </body>
    </html>
    "#,
        requested_id
    );

    Ok(Html(html))
}
async fn websocket_handler(
    Path(requested_id): Path<String>,
    ws: WebSocketUpgrade,
    State(state): State<Arc<AppState>>,
) -> Result<Response, axum::http::StatusCode> {
    // ✅ Validate the share ID
    if requested_id != state.share_id {
        return Err(axum::http::StatusCode::NOT_FOUND);
    }

    Ok(ws.on_upgrade(move |socket| handle_websocket(socket, state)))
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

    if let Err(e) = sender
        .send(Message::Text(initial_message.to_string().into()))
        .await
    {
        eprintln!("Failed to send initial message: {}", e);
        return;
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

                        if let Err(e) = sender.send(Message::Text(message.to_string().into())).await {
                            eprintln!("Failed to send update: {}", e);
                            break;
                        }
                    }
                    Err(broadcast::error::RecvError::Lagged(skipped)) => {
                        eprintln!("Client lagged, skipped {} messages", skipped);
                        continue;
                    }
                    Err(_) => break, // Channel closed
                }
            }

            // Handle client messages (heartbeat, etc.)
            msg = receiver.next() => {
                match msg {
                    Some(Ok(Message::Close(_))) => {
                        println!("Client closed connection gracefully");
                        break;
                    }
                    Some(Err(e)) => {
                        eprintln!("WebSocket error: {}", e);
                        break;
                    }
                    None => {
                        println!("Client connection ended");
                        break;
                    }
                    _ => {} // Ignore other messages like Ping/Pong
                }
            }
        }
    }

    // Client disconnected - cleanup
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
    share_id: String, // ✅ Store the generated share ID
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
