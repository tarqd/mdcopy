pub mod tools;

use crate::config::{
    CliArgs, CliHighlightArgs, CliImageArgs, CliMermaidArgs, Config, ImageConfig, MermaidConfig,
    MermaidFormat,
};
use crate::render::{self, RenderContext};
use rmcp::{
    ErrorData as McpError, Peer, RoleServer, ServerHandler,
    handler::server::router::tool::ToolRouter,
    handler::server::wrapper::Parameters,
    model::*,
    service::ElicitationError,
    tool, tool_handler, tool_router,
    ServiceExt,
};
use std::sync::Arc;
use tools::{RenderMermaidParams, RenderToClipboardParams, RenderToFileParams};

/// MCP server wrapping mdcopy's rendering capabilities.
#[derive(Clone)]
pub struct MdcopyMcpServer {
    config: Arc<Config>,
    render_ctx: Arc<RenderContext>,
    tool_router: ToolRouter<Self>,
}

#[tool_router]
impl MdcopyMcpServer {
    fn new() -> Self {
        let empty_cli = CliArgs {
            input: None,
            output: None,
            root: None,
            strict: None,
            prosemirror: None,
            highlight: CliHighlightArgs {
                enable: None,
                theme: None,
                themes_dir: None,
                syntaxes_dir: None,
            },
            image: CliImageArgs {
                embed_local: None,
                embed_remote: None,
                optimize_local: None,
                optimize_remote: None,
                max_dimension: None,
                quality: None,
            },
            mermaid: CliMermaidArgs {
                embed: None,
                format: Some(MermaidFormat::Svg), // SVG default for MCP (CLI defaults to PNG)
                optimize: None,
                max_width: None,
                max_height: None,
            },
        };
        let (config, _) = Config::build(empty_cli, None);
        let render_ctx = RenderContext::new(&config);

        Self {
            config: Arc::new(config),
            render_ctx: Arc::new(render_ctx),
            tool_router: Self::tool_router(),
        }
    }

    #[tool(
        description = "Render markdown to rich HTML and copy to the system clipboard. The clipboard will contain both HTML (for rich paste) and plain text. For local markdown files, prefer file_path over reading file content into the conversation."
    )]
    async fn render_markdown_to_clipboard(
        &self,
        Parameters(params): Parameters<RenderToClipboardParams>,
    ) -> Result<CallToolResult, McpError> {
        let config = self.config.clone();
        let render_ctx = self.render_ctx.clone();

        let result = tokio::task::spawn_blocking(move || {
            let (markdown_text, base_dir) = tools::resolve_input(
                params.file_path.as_deref(),
                params.text.as_deref(),
                params.root.as_deref(),
            )?;

            let image_config = ImageConfig {
                embed_local: params.embed_local,
                embed_remote: params.embed_remote,
                optimize_local: params.optimize && params.embed_local,
                optimize_remote: params.optimize && params.embed_remote,
                max_dimension: config.image.max_dimension,
                quality: config.image.quality,
            };

            let html = render::render_to_html(
                &markdown_text,
                &base_dir,
                &image_config,
                config.strict,
                config.prosemirror,
                &config.mermaid,
                &render_ctx,
            )?;

            render::copy_to_clipboard(&html, &markdown_text)?;

            Ok::<_, std::io::Error>(html.len())
        })
        .await
        .map_err(|e| McpError::internal_error(format!("Task join error: {}", e), None))?
        .map_err(|e| McpError::internal_error(format!("{}", e), None))?;

        Ok(CallToolResult::success(vec![Content::text(format!(
            "Copied to clipboard ({} bytes HTML)",
            result
        ))]))
    }

    #[tool(
        description = "Render markdown to a standalone HTML file. For local markdown files, prefer file_path over reading file content."
    )]
    async fn render_markdown_to_file(
        &self,
        Parameters(params): Parameters<RenderToFileParams>,
    ) -> Result<CallToolResult, McpError> {
        let config = self.config.clone();
        let render_ctx = self.render_ctx.clone();

        let result = tokio::task::spawn_blocking(move || {
            let (markdown_text, base_dir) = tools::resolve_input(
                params.file_path.as_deref(),
                params.text.as_deref(),
                params.root.as_deref(),
            )?;

            let image_config = ImageConfig {
                embed_local: params.embed_local,
                embed_remote: params.embed_remote,
                optimize_local: params.optimize && params.embed_local,
                optimize_remote: params.optimize && params.embed_remote,
                max_dimension: config.image.max_dimension,
                quality: config.image.quality,
            };

            let html = render::render_to_html(
                &markdown_text,
                &base_dir,
                &image_config,
                config.strict,
                config.prosemirror,
                &config.mermaid,
                &render_ctx,
            )?;

            let output_path = std::path::Path::new(&params.output_path);
            render::write_to_file(&html, output_path)?;

            Ok::<_, std::io::Error>((html.len(), params.output_path))
        })
        .await
        .map_err(|e| McpError::internal_error(format!("Task join error: {}", e), None))?
        .map_err(|e| McpError::internal_error(format!("{}", e), None))?;

        Ok(CallToolResult::success(vec![Content::text(format!(
            "Wrote {} bytes to {}",
            result.0, result.1
        ))]))
    }

    #[tool(
        description = "Render a mermaid diagram to an image file. Accepts mermaid source as text or a file path. Outputs SVG, PNG, or JPEG to a specified file or a temporary file."
    )]
    async fn render_mermaid(
        &self,
        peer: Peer<RoleServer>,
        Parameters(params): Parameters<RenderMermaidParams>,
    ) -> Result<CallToolResult, McpError> {
        // Validate mutual exclusivity of source/source_file
        if params.source.is_some() && params.source_file.is_some() {
            return Err(McpError::invalid_params(
                "source and source_file are mutually exclusive",
                None,
            ));
        }

        // Resolve mermaid source text
        let mermaid_source = match (&params.source_file, &params.source) {
            (Some(path), _) => std::fs::read_to_string(path).map_err(|e| {
                McpError::internal_error(format!("Failed to read source_file: {}", e), None)
            })?,
            (_, Some(text)) => text.clone(),
            (None, None) => {
                return Err(McpError::invalid_params(
                    "Either source or source_file must be provided",
                    None,
                ));
            }
        };

        // Resolve format from param or config default
        let format = match params.format.as_deref() {
            Some("svg") => MermaidFormat::Svg,
            Some("png") => MermaidFormat::Png,
            Some("jpeg" | "jpg") => MermaidFormat::Jpeg,
            Some(other) => {
                return Err(McpError::invalid_params(
                    format!("Invalid format '{}'. Expected: svg, png, or jpeg", other),
                    None,
                ));
            }
            None => self.config.mermaid.format,
        };

        let extension = format.to_string();

        // Resolve output path, checking overwrite logic
        let output_path = if let Some(ref path_str) = params.output_file {
            let path = std::path::PathBuf::from(path_str);
            if path.exists() && !params.overwrite {
                // Try elicitation
                let should_overwrite =
                    try_elicit_overwrite(&peer, path_str).await;
                if !should_overwrite {
                    return Ok(CallToolResult::error(vec![Content::text(format!(
                        "File already exists: {}. Set overwrite=true or confirm via elicitation.",
                        path_str
                    ))]));
                }
            }
            path
        } else {
            // Create a temp file
            let tmp = tempfile::Builder::new()
                .prefix("mermaid-")
                .suffix(&format!(".{}", extension))
                .tempfile()
                .map_err(|e| {
                    McpError::internal_error(
                        format!("Failed to create temp file: {}", e),
                        None,
                    )
                })?;
            let (_, path) = tmp.keep().map_err(|e| {
                McpError::internal_error(format!("Failed to persist temp file: {}", e), None)
            })?;
            path
        };

        let config = self.config.clone();
        let render_ctx = self.render_ctx.clone();
        let output_path_clone = output_path.clone();

        let result = tokio::task::spawn_blocking(move || {
            // Build configs from params + config defaults
            let mermaid_config = MermaidConfig {
                embed: true, // not relevant for direct rendering, but needed for the struct
                format,
                optimize: params.optimize.unwrap_or(config.mermaid.optimize),
                max_width: params.max_width.unwrap_or(config.mermaid.max_width),
                max_height: params.max_height.unwrap_or(config.mermaid.max_height),
            };

            let image_config = ImageConfig {
                max_dimension: config.image.max_dimension,
                quality: params.quality.unwrap_or(config.image.quality),
                ..Default::default()
            };

            // Render SVG from mermaid source
            let svg = mermaid_rs_renderer::render(&mermaid_source)
                .map_err(|e| std::io::Error::other(format!("Mermaid render failed: {}", e)))?;

            // Write output based on format
            match format {
                MermaidFormat::Svg => {
                    std::fs::write(&output_path_clone, &svg)?;
                    Ok::<_, std::io::Error>(svg.len())
                }
                MermaidFormat::Png | MermaidFormat::Jpeg => {
                    let fontdb = render_ctx.mermaid_cache.fontdb();
                    let raster = crate::mermaid::rasterize_svg_to_raster(
                        &svg,
                        &mermaid_config,
                        &image_config,
                        fontdb,
                    )
                    .map_err(|e| std::io::Error::other(format!("Rasterization failed: {}", e)))?;
                    let data_len = raster.image.data.len();
                    std::fs::write(&output_path_clone, &raster.image.data)?;
                    Ok(data_len)
                }
            }
        })
        .await
        .map_err(|e| McpError::internal_error(format!("Task join error: {}", e), None))?
        .map_err(|e| McpError::internal_error(format!("{}", e), None))?;

        Ok(CallToolResult::success(vec![Content::text(format!(
            "Wrote {} bytes to {}",
            result,
            output_path.display()
        ))]))
    }
}

/// Try to ask the user via elicitation whether they want to overwrite a file.
/// Returns true if user confirms, false otherwise (declined, cancelled, or not supported).
async fn try_elicit_overwrite(peer: &Peer<RoleServer>, path: &str) -> bool {
    use crate::mcp::tools::OverwriteConfirm;

    // Check if client supports elicitation
    if peer.supported_elicitation_modes().is_empty() {
        return false;
    }

    match peer
        .elicit::<OverwriteConfirm>(format!(
            "File already exists: {}. Overwrite?",
            path
        ))
        .await
    {
        Ok(Some(confirm)) => confirm.overwrite,
        Ok(None) | Err(ElicitationError::UserDeclined | ElicitationError::UserCancelled) => false,
        Err(_) => false,
    }
}

#[tool_handler]
impl ServerHandler for MdcopyMcpServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            protocol_version: ProtocolVersion::V_2025_03_26,
            capabilities: ServerCapabilities::builder()
                .enable_tools()
                .build(),
            server_info: Implementation {
                name: "mdcopy".to_string(),
                title: None,
                version: env!("CARGO_PKG_VERSION").to_string(),
                description: None,
                icons: None,
                website_url: None,
            },
            instructions: Some(
                "mdcopy renders Markdown to rich HTML for clipboard or file output. \
                 Tools: render_markdown_to_clipboard (copies formatted markdown to system clipboard), \
                 render_markdown_to_file (writes standalone HTML file), \
                 render_mermaid (renders mermaid diagram to SVG/PNG/JPEG file). \
                 For local files, pass file_path rather than file content to avoid bloating the conversation."
                    .to_string(),
            ),
        }
    }
}

/// Run the MCP server with the specified transport.
pub fn run_mcp_server(transport: &str, listen: &str) -> std::io::Result<()> {
    let rt = tokio::runtime::Runtime::new()?;
    rt.block_on(async {
        match transport {
            "stdio" => run_stdio().await,
            "http" => run_http(listen).await,
            other => Err(std::io::Error::other(format!(
                "Unknown transport: {} (expected stdio or http)",
                other
            ))),
        }
    })
}

async fn run_stdio() -> std::io::Result<()> {
    let server = MdcopyMcpServer::new();
    let service = server
        .serve(rmcp::transport::stdio())
        .await
        .map_err(std::io::Error::other)?;

    service.waiting().await.map_err(std::io::Error::other)?;
    Ok(())
}

async fn run_http(listen: &str) -> std::io::Result<()> {
    use rmcp::transport::streamable_http_server::{
        StreamableHttpServerConfig, StreamableHttpService,
        session::local::LocalSessionManager,
    };

    let ct = tokio_util::sync::CancellationToken::new();

    let service = StreamableHttpService::new(
        || Ok(MdcopyMcpServer::new()),
        LocalSessionManager::default().into(),
        StreamableHttpServerConfig {
            cancellation_token: ct.child_token(),
            ..Default::default()
        },
    );

    let router = axum::Router::new().nest_service("/mcp", service);
    let tcp_listener = tokio::net::TcpListener::bind(listen).await?;

    eprintln!("MCP server listening on http://{}/mcp", listen);

    axum::serve(tcp_listener, router)
        .with_graceful_shutdown(async move {
            tokio::signal::ctrl_c().await.unwrap();
            ct.cancel();
        })
        .await?;

    Ok(())
}
