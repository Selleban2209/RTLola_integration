use std::convert::Infallible;
use std::os::raw::{c_char, c_double, c_ulong, c_long, c_int, c_longlong, c_ulonglong, c_void};
use std::ffi::{CStr, CString};
use std::time::Instant;

use ordered_float::NotNan;
use rtlola_interpreter::Value;
use rtlola_monitor::RtlolaMonitor;
mod rtlola_monitor;
/*
#[repr(C)]
pub struct RTLolaMonitorHandle {
    monitor: QueuedMonitor<VectorFactory<Infallible, Vec<Value>>, OfflineMode<RelativeFloat>, TotalIncremental, RelativeFloat>,
    input_names: Vec<String>,
    start_time: Instant,
}
*/

#[repr(C)]
pub struct RTLolaMonitorHandle {
    inner: *mut c_void, // Opaque pointer to RtlolaMonitor
    
}

#[repr(C)]
pub struct RTLolaInput {
    name: *const c_char,
    type_: u32, // 0=UInt64, 1=Int64, 2=Float64, 3=Bool, 4=String
    value: RTLolaValueData,
}

#[repr(C)]
pub union RTLolaValueData {
    uint64_val: c_ulonglong,
    int64_val: c_longlong,
    float64_val: c_double,
    bool_val: bool,
    string_val: *const c_char,
}

#[unsafe(no_mangle)]
pub extern "C" fn rtlola_monitor_new(
    spec: *const c_char,
    timeout_ms: u64,
    input_names: *const *const c_char,
    num_inputs: u64
) -> *mut RTLolaMonitorHandle {
    // Convert the C spec string to Rust String
    let spec_cstr = unsafe { CStr::from_ptr(spec) };
    let spec_str = match spec_cstr.to_str() {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Failed to parse specification string: {}", e);
            return std::ptr::null_mut();
        }
    };

    // Convert the input names array
    let input_names_slice = unsafe { std::slice::from_raw_parts(input_names, num_inputs as usize) };
    let mut rust_input_names = Vec::with_capacity(num_inputs as usize);
    
    for &name_ptr in input_names_slice {
        let name_cstr = unsafe { CStr::from_ptr(name_ptr) };
        match name_cstr.to_str() {
            Ok(s) => rust_input_names.push(s),
            Err(e) => {
                eprintln!("Failed to parse input name: {}", e);
                return std::ptr::null_mut();
            }
        }
    }

    // Create the monitor instance
    let monitor = match RtlolaMonitor::new(spec_str, timeout_ms, &rust_input_names) {
        Ok(m) => m,
        Err(e) => {
            eprintln!("Failed to create monitor: {}", e);
            return std::ptr::null_mut();
        }
    };

    // Box the monitor to store on the heap
    let boxed_monitor = Box::new(monitor);
    
    // Create the handle with an opaque pointer to the monitor
    let handle = Box::new(RTLolaMonitorHandle {
        inner: Box::into_raw(boxed_monitor) as *mut c_void
    });

    // Return the raw pointer (caller now owns this)
    Box::into_raw(handle)
}

#[unsafe(no_mangle)]
pub extern "C" fn rtlola_process_inputs(
    handle: *mut RTLolaMonitorHandle,
    inputs: *const RTLolaInput,
    num_inputs: usize,
    time: c_double
) -> bool {
    if handle.is_null() || inputs.is_null() {
        return false;
    }

    let monitor = unsafe { &mut *( (*(handle as *mut RTLolaMonitorHandle)).inner as *mut RtlolaMonitor) };
    let inputs_slice = unsafe { std::slice::from_raw_parts(inputs, num_inputs) };

    let mut values = Vec::with_capacity(num_inputs);

    for input in inputs_slice {
        let value = match input.type_ {
            0 => Value::Unsigned(unsafe { input.value.uint64_val }),
            1 => Value::Signed(unsafe { input.value.int64_val }),
            2 => Value::Float(NotNan::try_from(unsafe { input.value.float64_val }).unwrap()),
            3 => Value::Bool(unsafe { input.value.bool_val }),
            4 => {
                let s = unsafe { CStr::from_ptr(input.value.string_val) };
                Value::Str(s.to_string_lossy().into_owned().into())
            },
            _ => return false, // Invalid type
        };
        values.push(value);
    }

    
    (*monitor).process_event_verdict(values).is_ok()
    
}

#[unsafe(no_mangle)]
pub extern "C" fn rtlola_monitor_start(handle: *mut RTLolaMonitorHandle) -> bool {
    let handle = unsafe { &mut *handle };
    let monitor = unsafe { &mut *(handle.inner as *mut RtlolaMonitor) };
    monitor.start().is_ok()
}

#[unsafe(no_mangle)]
pub extern "C" fn rtlola_monitor_free(handle: *mut RTLolaMonitorHandle) {
    if !handle.is_null() {
        unsafe { Box::from_raw(handle) };
    }
}