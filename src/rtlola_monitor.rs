use std::collections::HashMap;
use std::time::{Duration, Instant};
use std::convert::Infallible;
use ordered_float::Float;
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

    pub fn process_event(&mut self, inputs: Vec<Value>, current_time: Option<std::time::Duration> ) -> Result<QueuedVerdict<TotalIncremental, RelativeFloat>, String> {
        if inputs.len() != self.input_names.len() {
            return Err(format!(
                "Expected {} inputs, got {}",
                self.input_names.len(),
                inputs.len()
            ));
        }
        /*
        i want it so that if this function is calle dike this 
        self.process_event(inputs,Null)?;
        then let elapsed = self.start_time.elapsed();
        
        but if 
        self.process_event(inputs,current_time)?;
        then let elapsed = current_time;

        
         */

        
        let elapsed = match current_time {
        Some(time) => time,
        None => self.start_time.elapsed(),
        };
        let elapsed = self.start_time.elapsed();

        let test: u64 = 20.0 as u64;

        self.monitor.accept_event(inputs, elapsed)
            .map_err(|e| format!("Failed to accept event: {:?}", e))?;
            
        self.receiver.recv_timeout(self.timeout)
            .map_err(|e| match e {
                crossbeam_channel::RecvTimeoutError::Timeout => "Timeout while waiting for verdict".to_string(),
                crossbeam_channel::RecvTimeoutError::Disconnected => "Monitor channel disconnected".to_string(),
            })
    }

    pub fn process_event_verdict(&mut self, inputs: Vec<Value>, current_time: Option<f64> ) -> Result<String, String> {
        let elapsed = match current_time {
            Some(seconds) => std::time::Duration::from_secs_f64(seconds),
            None => self.start_time.elapsed()
        };
        let verdict = self.process_event(inputs,Some(elapsed))?;
        let ir = self.monitor.ir();
        let ts = elapsed.as_secs_f64();
        
       
        // Main output string with color codes
        let mut string_output = String::new();
        
        match verdict.kind {
            VerdictKind::Timed => {
                string_output.push_str(&format!(
                    "{} {}\n",
                    format!("[{:.6}s]", ts),
                    "[Trigger] Deadline reached".red().to_string()
                ));
            },
            VerdictKind::Event => {
                string_output.push_str(&format!(
                    "{} {}\n",
                    format!("[{:.6}s]", ts),
                    "Processing new event"
                ));
                
                for (idx, val) in verdict.verdict.inputs {
                    let input = &ir.inputs[idx];
                    string_output.push_str(&format!(
                        "{} {} {} {}\n",
                        format!("[{:.6}s]", ts),
                        "[Input]".cyan().to_string(),
                        format!("[{}]", input.name).cyan().to_string(),
                        format!("= {}", self.format_number(val, Self::DEFAULT_THRESHOLD))
                    ));
                }
            },
        }
    
        for (out_idx, changes) in verdict.verdict.outputs {
            let output = &ir.outputs[out_idx];
            let (prefix, name) = match &output.kind {
                OutputKind::NamedOutput(name) => {
                    ("Output", format!("[Output][{}]", name).blue().to_string())
                },
                OutputKind::Trigger(trigger_idx) => {
                    ("Trigger", format!("[#{}]", trigger_idx).red().to_string())
                },
            };
    
            for change in changes {
                match change {
                    Change::Spawn(param) => {
                        string_output.push_str(&format!(
                            "{} {} {} {:?}\n",
                            format!("[{:.6}s]", ts),
                            name,
                            "[Spawn]".purple().to_string(),
                            param
                        ));
                    },
                    Change::Value(param, val) => {
                        let is_output = matches!(output.kind, OutputKind::NamedOutput(_));
                        let is_trigger = matches!(output.kind, OutputKind::Trigger(_));
                       
                        if is_output {
                            string_output.push_str(&format!(
                                "{} {} {} {}\n",
                                format!("[{:.6}s]", ts),
                                name,
                                "[Value] = ".green().to_string(),
                                self.format_number(val.clone(), Self::DEFAULT_THRESHOLD)
                            ));
                        }   
                        
                        if is_trigger {
                            string_output.push_str(&format!(
                                "{} {} {} {}\n",
                                format!("[{:.6}s]", ts),
                                "[Trigger]".red().to_string(),
                                name,
                                format!("= {}", val)
                            ));
                        }
                    },
                    Change::Close(param) => {
                        string_output.push_str(&format!(
                            "{} {} {} {:?}\n",
                            format!("[{:.6}s]", ts),
                            name,
                            "[Close]".yellow().to_string(),
                            param
                        ));
                    },
                }
            }
        }
    
        Ok(string_output)  // Explicitly return our built string
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