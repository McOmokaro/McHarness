//! CDP (Chrome DevTools Protocol) Client using chromiumoxide 0.9

use chromiumoxide::{Browser, BrowserConfig, Page};
use chromiumoxide::cdp::browser_protocol::page::CaptureScreenshotFormat;
use chromiumoxide::cdp::browser_protocol::target::CreateTargetParams;
use chromiumoxide::error::CdpError as ChromiumOxideError;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info, warn};
use uuid::Uuid;

/// CDP client configuration
#[derive(Debug, Clone)]
pub struct CdpConfig {
    /// Chrome/Chromium executable path
    pub chrome_path: Option<String>,
    /// Remote debugging port (if connecting to existing instance)
    pub remote_port: Option<u16>,
    /// Headless mode
    pub headless: bool,
    /// Window size
    pub window_size: (u32, u32),
    /// User data directory
    pub user_data_dir: Option<String>,
    /// Additional Chrome args
    pub extra_args: Vec<String>,
}

impl Default for CdpConfig {
    fn default() -> Self {
        Self {
            chrome_path: None,
            remote_port: None,
            headless: true,
            window_size: (1920, 1080),
            user_data_dir: None,
            extra_args: vec![
                "--disable-blink-features=AutomationControlled".to_string(),
                "--disable-dev-shm-usage".to_string(),
                "--no-sandbox".to_string(),
            ],
        }
    }
}

/// CDP client wrapper
pub struct CdpClient {
    browser: Browser,
    pages: Arc<RwLock<HashMap<Uuid, Page>>>,
    config: CdpConfig,
}

impl CdpClient {
    /// Launch a new browser instance
    pub async fn launch(config: CdpConfig) -> Result<Self, CdpError> {
        let mut browser_config = BrowserConfig::builder();

        if let Some(path) = &config.chrome_path {
            browser_config = browser_config.chrome_executable(path);
        }

        if let Some(port) = config.remote_port {
            browser_config = browser_config.port(port);
        }

        if !config.headless {
            browser_config = browser_config.with_head();
        }

        browser_config = browser_config.window_size(config.window_size.0, config.window_size.1);

        if let Some(dir) = &config.user_data_dir {
            browser_config = browser_config.user_data_dir(dir.clone());
        }

        for arg in &config.extra_args {
            browser_config = browser_config.arg(arg.clone());
        }

        let built_config = browser_config.build().map_err(|e| CdpError::Launch(ChromiumOxideError::Io(std::io::Error::new(std::io::ErrorKind::Other, e))))?;
        let (browser, _handler) = Browser::launch(built_config).await?;

        info!("CDP browser launched");

        Ok(Self {
            browser,
            pages: Arc::new(RwLock::new(HashMap::new())),
            config,
        })
    }

    /// Create a new page
    pub async fn new_page(&self) -> Result<PageHandle, CdpError> {
        let page = self.browser.new_page(CreateTargetParams::default()).await?;
        let handle = PageHandle {
            id: Uuid::new_v4(),
            page,
        };
        self.pages.write().await.insert(handle.id, handle.page.clone());
        Ok(handle)
    }

    /// Get page by ID
    pub async fn get_page(&self, id: Uuid) -> Option<PageHandle> {
        self.pages.read().await.get(&id).map(|p| PageHandle {
            id,
            page: p.clone(),
        })
    }

    /// Close a page
    pub async fn close_page(&self, id: Uuid) -> Result<(), CdpError> {
        if let Some(page) = self.pages.write().await.remove(&id) {
            page.close().await?;
        }
        Ok(())
    }

    /// Take screenshot
    pub async fn screenshot(&self, page_id: Uuid) -> Result<Vec<u8>, CdpError> {
        let page = self.pages.read().await.get(&page_id).cloned()
            .ok_or(CdpError::PageNotFound(page_id))?;

        let screenshot = page
            .screenshot(
                chromiumoxide::page::ScreenshotParams::builder()
                    .format(CaptureScreenshotFormat::Png)
                    .build()
            )
            .await?;

        Ok(screenshot)
    }

    /// Navigate to URL
    pub async fn navigate(&self, page_id: Uuid, url: &str) -> Result<(), CdpError> {
        let page = self.pages.read().await.get(&page_id).cloned()
            .ok_or(CdpError::PageNotFound(page_id))?;

        page.goto(url).await?;
        page.wait_for_navigation().await?;
        Ok(())
    }

    /// Execute JavaScript
    pub async fn evaluate(&self, page_id: Uuid, script: &str) -> Result<serde_json::Value, CdpError> {
        let page = self.pages.read().await.get(&page_id).cloned()
            .ok_or(CdpError::PageNotFound(page_id))?;

        let result = page.evaluate(script).await?;
        let value = result.into_value()?;
        Ok(value)
    }

    /// Click at coordinates
    pub async fn click(&self, page_id: Uuid, x: f64, y: f64) -> Result<(), CdpError> {
        let page = self.pages.read().await.get(&page_id).cloned()
            .ok_or(CdpError::PageNotFound(page_id))?;

        use chromiumoxide::cdp::browser_protocol::input::{DispatchMouseEventParams, MouseButton, DispatchMouseEventType};

        // Mouse pressed
        page.execute(
            DispatchMouseEventParams::builder()
                .x(x)
                .y(y)
                .button(MouseButton::Left)
                .click_count(1)
                .r#type(DispatchMouseEventType::MousePressed)
                .build().map_err(|e| CdpError::Click(e))?
        ).await?;

        // Mouse released
        page.execute(
            DispatchMouseEventParams::builder()
                .x(x)
                .y(y)
                .button(MouseButton::Left)
                .click_count(1)
                .r#type(DispatchMouseEventType::MouseReleased)
                .build().map_err(|e| CdpError::Click(e))?
        ).await?;

        Ok(())
    }

    /// Type text
    pub async fn type_text(&self, page_id: Uuid, text: &str) -> Result<(), CdpError> {
        let page = self.pages.read().await.get(&page_id).cloned()
            .ok_or(CdpError::PageNotFound(page_id))?;

        use chromiumoxide::cdp::browser_protocol::input::{DispatchKeyEventParams, DispatchKeyEventType};

        for ch in text.chars() {
            page.execute(
                DispatchKeyEventParams::builder()
                    .r#type(DispatchKeyEventType::Char)
                    .text(ch.to_string())
                    .build().map_err(|e| CdpError::Type(e))?
            ).await?;

            tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
        }
        Ok(())
    }

    /// Scroll
    pub async fn scroll(&self, page_id: Uuid, dx: f64, dy: f64) -> Result<(), CdpError> {
        let page = self.pages.read().await.get(&page_id).cloned()
            .ok_or(CdpError::PageNotFound(page_id))?;

        use chromiumoxide::cdp::browser_protocol::input::{DispatchMouseEventParams, DispatchMouseEventType};

        page.execute(
            DispatchMouseEventParams::builder()
                .x(0.0)
                .y(0.0)
                .delta_x(dx)
                .delta_y(dy)
                .r#type(chromiumoxide::cdp::browser_protocol::input::DispatchMouseEventType::MouseWheel)
                .build().map_err(|e| CdpError::Click(e))?
        ).await?;
        Ok(())
    }

    /// Get accessibility tree (simplified - returns empty for now)
    pub async fn get_accessibility_tree(&self, _page_id: Uuid) -> Result<AccessibilityTree, CdpError> {
        Ok(AccessibilityTree::empty())
    }

    /// Close browser
    pub async fn close(&mut self) -> Result<(), CdpError> {
        self.browser.close().await?;
        Ok(())
    }
}

/// Page handle with ID
#[derive(Clone)]
pub struct PageHandle {
    pub id: Uuid,
    pub page: Page,
}

/// Accessibility tree node
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AxNode {
    pub node_id: String,
    pub role: String,
    pub name: Option<String>,
    pub description: Option<String>,
    pub value: Option<String>,
    pub bounds: Option<AxBounds>,
    pub focused: bool,
    pub selected: bool,
    pub disabled: bool,
    pub editable: bool,
    pub children: Vec<AxNode>,
    pub attributes: HashMap<String, String>,
}

/// Accessibility tree bounds
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AxBounds {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

/// Full accessibility tree
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccessibilityTree {
    pub nodes: Vec<AxNode>,
    pub root_id: String,
    pub focused_node_id: Option<String>,
}

impl AccessibilityTree {
    /// Create empty tree (accessibility tree via CDP not directly available in chromiumoxide 0.9)
    pub fn empty() -> Self {
        Self {
            nodes: Vec::new(),
            root_id: String::new(),
            focused_node_id: None,
        }
    }
}

/// CDP errors
#[derive(Debug, thiserror::Error)]
pub enum CdpError {
    #[error("Browser launch failed: {0}")]
    Launch(#[from] ChromiumOxideError),

    #[error("Page not found: {0}")]
    PageNotFound(Uuid),

    #[error("Navigation failed: {0}")]
    Navigation(String),

    #[error("Script evaluation failed: {0}")]
    Evaluation(String),

    #[error("Script evaluation failed: {0}")]
    Json(#[from] serde_json::Error),

    #[error("Screenshot failed: {0}")]
    Screenshot(String),

    #[error("Accessibility tree failed: {0}")]
    AccessibilityTree(String),

    #[error("Click failed: {0}")]
    Click(String),

    #[error("Type failed: {0}")]
    Type(String),
}