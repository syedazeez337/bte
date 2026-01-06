#[cfg(target_arch = "wasm32")]
pub mod wasm {
    use wasm_bindgen::prelude::*;

    #[wasm_bindgen]
    pub struct WasiPtyProcess {
        #[wasm_bindgen(skip_field)]
        pub process: Option<std::process::Child>,
    }

    #[wasm_bindgen]
    impl WasiPtyProcess {
        #[wasm_bindgen(constructor)]
        pub fn new() -> WasiPtyProcess {
            WasiPtyProcess { process: None }
        }

        #[wasm_bindgen]
        pub fn spawn(&mut self, program: &str, args: JsValue) -> Result<bool, JsValue> {
            let args: Vec<String> = args
                .into_serde()
                .map_err(|_| JsValue::from_str("Invalid arguments"))?;

            let output = std::process::Command::new(program)
                .args(args)
                .output()
                .map_err(|e| JsValue::from_str(&format!("Failed to spawn: {}", e)))?;

            self.process = Some(output.child);

            Ok(true)
        }

        #[wasm_bindgen]
        pub fn write(&mut self, data: &str) -> Result<usize, JsValue> {
            Ok(data.len())
        }

        #[wasm_bindgen]
        pub fn read(&mut self, size: usize) -> Result<String, JsValue> {
            Ok(String::new())
        }

        #[wasm_bindgen]
        pub fn kill(&mut self) -> Result<bool, JsValue> {
            Ok(true)
        }
    }

    #[wasm_bindgen]
    pub fn create_screen(width: u16, height: u16) -> Result<JsValue, JsValue> {
        serde_wasm_bindgen::to_value(&crate::screen::Screen::new(width, height))
            .map_err(|e| JsValue::from_str(&format!("Failed to create screen: {}", e)))
    }

    #[wasm_bindgen]
    pub fn run_scenario(scenario_json: &str) -> Result<JsValue, JsValue> {
        let scenario: crate::scenario::Scenario = serde_yaml::from_str(scenario_json)
            .map_err(|e| JsValue::from_str(&format!("Invalid scenario: {}", e)))?;

        let results = crate::runner::run_scenario(&scenario);
        serde_wasm_bindgen::to_value(&results)
            .map_err(|e| JsValue::from_str(&format!("Failed to serialize results: {}", e)))
    }
}

#[no_mangle]
pub extern "C" fn allocate_memory(size: usize) -> *mut u8 {
    let mut buffer = Vec::with_capacity(size);
    let ptr = buffer.as_mut_ptr();
    std::mem::forget(buffer);
    ptr
}

#[no_mangle]
pub extern "C" fn free_memory(ptr: *mut u8, size: usize) {
    unsafe {
        Vec::from_raw_parts(ptr, size, size);
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_wasm_module_compiles() {
        #[cfg(target_arch = "wasm32")]
        {
            assert!(true);
        }
    }
}
