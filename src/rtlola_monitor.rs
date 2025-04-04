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
use colored::*;

pub struct RtlolaMonitor {
    start_time: Instant,
    monitor: QueuedMonitor<VectorFactory<Infallible, Vec<Value>>, OfflineMode<RelativeFloat>, TotalIncremental, RelativeFloat>,
    timeout: Duration,
    receiver: Receiver<QueuedVerdict<TotalIncremental, RelativeFloat>>,
    input_names: Vec<String>, // Track input names for validation
}

impl RtlolaMonitor {
    
    const DEFAULT_THRESHOLD: f64 = 1e-6;

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
                println!(
                    "{} {}",
                    format!("[{:.6}s]", ts),
                    "[Trigger] Deadline reached".red()
                );
            },
            VerdictKind::Event => {
                println!(
                    "{} {}",
                    format!("[{:.6}s]", ts),
                    "Processing new event"
                );
                
                for (idx, val) in verdict.verdict.inputs {
                    let input = &ir.inputs[idx];
                    println!(
                        "{} {} {} {}",
                        format!("[{:.6}s]", ts),
                        "[Input]".cyan(),
                        format!("[{}]", input.name).cyan(),
                        format!("= {}", self.format_number(val, Self::DEFAULT_THRESHOLD))
                    );
                }
            },
        }
    
        // Print outputs and triggers - matching the interpreter's style
        for (out_idx, changes) in verdict.verdict.outputs {
            let output = &ir.outputs[out_idx];
            let (prefix, name) = match &output.kind {
                OutputKind::NamedOutput(name) => {
                    ("Output", format!("[Output][{}]", name).blue())
                },
                OutputKind::Trigger(trigger_idx) => {
                    ("Trigger", format!("[#{}]", trigger_idx).red())
                },
            };
    
            for change in changes {
                match change {
                    Change::Spawn(param) => {
                        println!(
                            "{} {} {} {:?}",
                            format!("[{:.6}s]", ts),
                            name,
                            "[Spawn]".purple(),
                            param
                        );
                    },
                    Change::Value(param, val) => {
                        let is_output = matches!(output.kind, OutputKind::NamedOutput(_));
                        let is_trigger = matches!(output.kind, OutputKind::Trigger(_));
                       
                        if is_output {
                            println!(
                                "{} {} {} {}",
                                format!("[{:.6}s]", ts),
                                name,
                                "[Value] = ".green(),
                                self.format_number(val.clone(), Self::DEFAULT_THRESHOLD)
                            );
                        }   
                        
                        if is_trigger {
                            println!(
                                "{} {} {} {}",
                                format!("[{:.6}s]", ts),
                                "[Trigger]".red(),
                                name,
                                format!("= {}", val)
                            );
                        }
                    },
                    Change::Close(param) => {
                        println!(
                            "{} {} {} {:?}",
                            format!("[{:.6}s]", ts),
                            name,
                            "[Close]".yellow(),
                            param
                        );
                    },
                }
            }
        }
    
        Ok(())
    }


    pub fn format_number(&self, val: Value, threshold: f64) -> String {
        match val {
            Value::Float(f) => {
                if f.abs() < threshold.abs() && f.abs() > 1e-10 {  // Handle negative thresholds
                    if f == 0.0 {
                        "0".to_string()
                    } else {
                        format!("{:.6e}", f.into_inner())
                    }
                }
                else if f.abs() < 1e-10 {
                    format!("{:.1}", 0.0)
                }    
                 else {
                    format!("{:.6}", f)
                }
            },
            // Other variants remain the same
            _ => val.to_string(),
        }
    }

}