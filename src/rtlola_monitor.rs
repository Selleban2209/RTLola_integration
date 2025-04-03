use std::collections::HashMap;
use std::time::{Duration, Instant};
use std::convert::Infallible;
use rtlola_frontend::mir::InputReference;
use rtlola_frontend::ParserConfig;
use rtlola_interpreter::input::VectorFactory;
use rtlola_interpreter::{
    monitor::{Change, TotalIncremental},
    config::OfflineMode,
    queued::{QueuedMonitor, QueuedVerdict, VerdictKind},
    time::RelativeFloat,
    ConfigBuilder, Value ,
    rtlola_mir::OutputKind, 
};
use std::fs;
use crossbeam_channel::Receiver;
pub struct RtlolaMonitor {
    start_time: Instant,
    monitor: QueuedMonitor<VectorFactory<Infallible, Vec<Value>>, OfflineMode<RelativeFloat>, TotalIncremental, RelativeFloat>,
    timeout: Duration,
    receiver: Receiver<QueuedVerdict<TotalIncremental, RelativeFloat>>,
    input_names: Vec<String>, // Track input names for validation
}

impl RtlolaMonitor {
    pub fn new(spec_path: &str, timeout_ms: u64, input_names: &[&str]) -> Result<Self, String> {


        let spec = fs::read_to_string(spec_path)
        .map_err(|e| format!("Failed to read specification file {}: {}", spec_path, e))?;
        // Parse spec and validate input count matches
        let ir = ParserConfig::for_string(spec.to_string()).parse()
            .map_err(|e| format!("Failed to parse specification: {:?}", e))?;

        if ir.inputs.len() != input_names.len() {
            return Err(format!(
                "Spec requires {} inputs but {} names provided",
                ir.inputs.len(),
                input_names.len()
            ));
        }

        // Create input mapping
        
        let map: HashMap<String, InputReference> = input_names
            .iter()
            .enumerate()
            .map(|(i, name)| {
                // Convert index to u32 (common type for InputReference)
                let index = i.try_into().expect("Too many inputs for u32");
                (name.to_string(),index)
            })
            .collect();

        let monitor = ConfigBuilder::new()
            .spec_str(&spec)
            .offline::<RelativeFloat>()
            .with_event_factory::<VectorFactory<Infallible, Vec<Value>>>()
            .with_verdict::<TotalIncremental>()
            .queued_monitor_with_data(input_names.len());
        
        let receiver = monitor.output_queue().clone();

        Ok(Self {
            start_time: Instant::now(),
            monitor,
            timeout: Duration::from_millis(timeout_ms),
            receiver,
            input_names: input_names.iter().map(|s| s.to_string()).collect(),
        })
    }

    pub fn start(&mut self) -> Result<(), String> {
        self.monitor.start()
            .map_err(|e| format!("Failed to start monitor: {:?}", e))
    }

    pub fn process_event(&mut self, inputs: Vec<Value>) -> Result<QueuedVerdict<TotalIncremental, RelativeFloat>, String> {
        if inputs.len() != self.input_names.len() {
            return Err(format!(
                "Expected {} inputs, got {}",
                self.input_names.len(),
                inputs.len()
            ));
        }

        let elapsed = self.start_time.elapsed();
        self.monitor.accept_event(inputs, elapsed)
            .map_err(|e| format!("Failed to accept event: {:?}", e))?;
            
        self.receiver.recv_timeout(self.timeout)
            .map_err(|e| match e {
                crossbeam_channel::RecvTimeoutError::Timeout => "Timeout while waiting for verdict".to_string(),
                crossbeam_channel::RecvTimeoutError::Disconnected => "Monitor channel disconnected".to_string(),
            })
    }

    pub fn process_event_with_cli_output(&mut self, inputs: Vec<Value>) -> Result<(), String> {
        let elapsed = self.start_time.elapsed();
        let ts = elapsed.as_secs_f64();

        let verdict = self.process_event(inputs)?;
        let ir = self.monitor.ir();
        
        match verdict.kind {
            VerdictKind::Timed => {
                println!("[{:.6}s][Trigger] Deadline reached", ts);
            },
            VerdictKind::Event => {
                println!("[{:.6}s] Processing new event", ts);
                
                for (idx, val) in verdict.verdict.inputs {
                    let input = &ir.inputs[idx];
                    println!("[{:.6}s][Input][{}][Value] = {}", ts, input.name, val);
                }
            },
        }

          // Print outputs and triggers - matching the interpreter's style
          for (out_idx, changes) in verdict.verdict.outputs {
            let output = &ir.outputs[out_idx];
            let (prefix, name) = match &output.kind {
                OutputKind::NamedOutput(name) => {
                    ("Output", format!("[Output][{}", name))
                },
                OutputKind::Trigger(trigger_idx) => {
                    ("Trigger", format!("[#{}", trigger_idx))
                },
            };

            for change in changes {
                match change {
                    Change::Spawn(param) => {
                        println!("[{:.6}s]{}][Spawn] = {:?}", ts, name, param);
                    },
                    Change::Value(param, val) => {
                        let is_output =  matches!(output.kind, OutputKind::NamedOutput(_));
                        let is_trigger = matches!(output.kind, OutputKind::Trigger(_));
                       
                        if is_output {
                            //print!("[{:.6}s]{}][Value] = ", ts, name);
                            println!("[{:.6}s]{}][Value] = {}", ts, name, val);
                        }   
                        // Handle trigger messages
                        if is_trigger {
                         //print!("[{:.6}s]{}][Trigger] = ", ts, name);
                           println!("[{:.6}s][Trigger]{}][Value] = {}", ts, name, val);
                        }
                    },
                    Change::Close(param) => {
                        println!("[{:.6}s]{}][Close] = {:?}", ts, name, param);
                    },
                }
            }
        }

        Ok(())
    }


}