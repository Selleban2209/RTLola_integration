mod rtlola_monitor;
use std::fs;

use rtlola_interpreter::{monitor::Incremental, queued::VerdictKind, Value};
use rtlola_monitor::RtlolaMonitor;
use ordered_float::{Float, NotNan};

use std::os::raw::{c_char,c_int, c_double, c_uint ,c_ulong, c_long };


fn main() -> Result<(), String> {
    // Example specification monitoring ball height
    let spec_file = "src/ball_spec.lola";
  
    // Create monitor with dynamic inputs
    let mut monitor = RtlolaMonitor::new(&spec_file, 500, &["height", "velocity", "temperature"])?;
    monitor.start()?;

    // Test data: (height, velocity, temperature, description)
    let test_data = vec![
        // Ball being thrown up (positive velocity)
        ([1.5, 2.5, 25.0], "Throw upwards (normal temp)"),
        ([3.0, 1.8, 28.0], "Ascending (warming)"),
        // Peak
        ([4.2, 0.0, 29.9], "At peak (almost hot)"),
        // Falling down
        ([3.5, -0.8, 31.0], "Starting descent (now hot)"),
        ([1.8, -2.2, 32.5], "Falling fast (hot)"),
        // Near ground
        ([0.3, -1.5, 33.0], "Approaching ground (hot)"),
        ([0.1, -0.5, 34.0], "Very close to ground"),
        ([0.0, 0.0, 35.0], "Impact with ground"),
        ([0.1, 1.0, 36.0], "Bounce (extremely hot)"),
    ];

    println!("Starting ball trajectory monitoring...\n");

    for (i, ([height, velocity, temp], desc)) in test_data.iter().enumerate() {
        println!("=== Event {}: {} ===", i + 1, desc);
        println!("Height: {:.2}m, Velocity: {:.2}m/s, Temp: {:.1}°C", 
                height, velocity, temp);

        let inputs = vec![
            Value::Float(NotNan::try_from(*height).unwrap()),
            Value::Float(NotNan::new(*velocity).unwrap()),
            Value::Float(NotNan::new(*temp).unwrap()),
        ];
        
        monitor.process_event_verdict(inputs)?;
        println!(); // Spacing
    }

    println!("Monitoring complete.");
    Ok(())
}

//[1.000000000][Trigger][#2][Value] = Ball is currently above ground