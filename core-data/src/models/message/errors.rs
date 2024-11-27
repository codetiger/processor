#[derive(Debug)]
pub struct FunctionResponseError {
    pub function: String,
    pub code: u32,
    pub message: String,
}

impl FunctionResponseError {
    pub fn new(function: String, code: u32, message: String) -> Self {
        FunctionResponseError { function, code, message }
    }
}
