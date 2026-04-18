use anyhow::Result;
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Stage {
    Compile,
    Clean,
    Backtest,
    Extract,
    Analyze,
    Done,
}

impl Stage {
    pub fn as_str(&self) -> &'static str {
        match self {
            Stage::Compile => "COMPILE",
            Stage::Clean => "CLEAN",
            Stage::Backtest => "BACKTEST",
            Stage::Extract => "EXTRACT",
            Stage::Analyze => "ANALYZE",
            Stage::Done => "DONE",
        }
    }

    pub fn next(&self) -> Option<Stage> {
        match self {
            Stage::Compile => Some(Stage::Clean),
            Stage::Clean => Some(Stage::Backtest),
            Stage::Backtest => Some(Stage::Extract),
            Stage::Extract => Some(Stage::Analyze),
            Stage::Analyze => Some(Stage::Done),
            Stage::Done => None,
        }
    }
}

pub struct StageExecutor;

impl StageExecutor {
    pub fn new() -> Self {
        Self
    }

    pub fn execute(&self, stage: Stage) -> Result<StageResult> {
        match stage {
            Stage::Compile => self.execute_compile(),
            Stage::Clean => self.execute_clean(),
            Stage::Backtest => self.execute_backtest(),
            Stage::Extract => self.execute_extract(),
            Stage::Analyze => self.execute_analyze(),
            Stage::Done => Ok(StageResult::complete()),
        }
    }

    fn execute_compile(&self) -> Result<StageResult> {
        Ok(StageResult::success())
    }

    fn execute_clean(&self) -> Result<StageResult> {
        Ok(StageResult::success())
    }

    fn execute_backtest(&self) -> Result<StageResult> {
        Ok(StageResult::success())
    }

    fn execute_extract(&self) -> Result<StageResult> {
        Ok(StageResult::success())
    }

    fn execute_analyze(&self) -> Result<StageResult> {
        Ok(StageResult::success())
    }
}

pub struct StageResult {
    pub success: bool,
    pub message: String,
    pub output: Option<PathBuf>,
}

impl StageResult {
    pub fn success() -> Self {
        Self {
            success: true,
            message: String::new(),
            output: None,
        }
    }

    pub fn complete() -> Self {
        Self {
            success: true,
            message: "Pipeline complete".to_string(),
            output: None,
        }
    }

    pub fn error(message: impl Into<String>) -> Self {
        Self {
            success: false,
            message: message.into(),
            output: None,
        }
    }
}
