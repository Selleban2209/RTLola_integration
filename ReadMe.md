# RTLolaMonitor FFI Library

This library provides an interface for interacting with the RTLola monitoring system using Rust code, which is exposed to C via FFI (Foreign Function Interface). It allows a C program to create, manipulate, and interact with the RTLolaMonitor, which processes events, generates verdicts, and manages input/output data based on a specified RTLola specification.

## Overview

The RTLolaMonitor is designed to evaluate real-time data streams according to an RTLola specification, and it is built to be accessed as a shared library from C. This Rust code exposes the functionality via FFI, enabling seamless integration between C and Rust codebases. The core purpose is to monitor events and generate verdicts based on real-time data inputs.

## Features

* **Event Processing**: Accept and process events, providing verdicts based on specified rules.
* **Timeout Support**: Timeout behavior to handle long-running events.
* **Input and Output Management**: Ability to handle multiple types of inputs and outputs, such as integers, floats, booleans, and strings.
* **Verdict Generation**: Generate detailed outputs that represent the evaluation of events over time.
* **C-Compatible API**: The library exposes C-callable functions to interface with C code via FFI.

## Building

To compile and build the shared library, ensure that you have `cargo` (Rust’s package manager and build tool) installed. You can follow the steps below to build the library:

1. **Clone the repository**:

   ```bash
   git clone <repo-url>
   cd <repo-directory>
   ```

2. **Build the library**:

   ```bash
   cargo build --release
   ```

   This will produce a shared library file (e.g., `librtlola_monitor.so` on Linux or `rtlola_monitor.dll` on Windows) that can be used in a C project.

## C API

This library exposes the following C-compatible functions for interacting with the RTLolaMonitor:

### `rtlola_monitor_new`

```c
RTLolaMonitorHandle* rtlola_monitor_new(
    const char* spec, 
    uint64_t timeout_ms, 
    const char** input_names, 
    uint64_t num_inputs
);
```

* **Parameters**:

  * `spec`: Path to the RTLola specification file (a string).
  * `timeout_ms`: Timeout value in milliseconds.
  * `input_names`: An array of input names (strings) for the monitor.
  * `num_inputs`: The number of inputs.
* **Returns**: A pointer to a new `RTLolaMonitorHandle` on success, or `NULL` on failure.

### `rtlola_process_inputs`

```c
char* rtlola_process_inputs(
    RTLolaMonitorHandle* handle, 
    RTLolaInput* inputs, 
    size_t num_inputs, 
    double time
);
```

* **Parameters**:

  * `handle`: A pointer to the `RTLolaMonitorHandle` created via `rtlola_monitor_new`.
  * `inputs`: A pointer to an array of `RTLolaInput` structures representing the inputs to process.
  * `num_inputs`: The number of inputs to process.
  * `time`: The current time for the event in seconds (as a `double`).
* **Returns**: A pointer to a string (C-style) representing the verdict or an error message. The caller is responsible for freeing the string using `rtlola_free_string`.

### `rtlola_monitor_start`

```c
bool rtlola_monitor_start(RTLolaMonitorHandle* handle);
```

* **Parameters**:

  * `handle`: A pointer to the `RTLolaMonitorHandle` created via `rtlola_monitor_new`.
* **Returns**: `true` if the monitor started successfully, `false` otherwise.

### `rtlola_monitor_free`

```c
void rtlola_monitor_free(RTLolaMonitorHandle* handle);
```

* **Parameters**:

  * `handle`: A pointer to the `RTLolaMonitorHandle` to be freed.

* **Returns**: None.

### `rtlola_free_string`

```c
void rtlola_free_string(char* str);
```

* **Parameters**:

  * `str`: A pointer to a C-style string to be freed.

* **Returns**: None.

## Example Usage in C

Here's a basic example of how to use the library from C:

```c
#include <stdio.h>
#include "rtlola_monitor.h"

int main() {
    // Example specification and input names
    const char* spec = "path/to/rtlola_spec.lola";
    const char* input_names[] = {"input1", "input2"};
    
    // Create the monitor
    RTLolaMonitorHandle* handle = rtlola_monitor_new(spec, 1000, input_names, 2);
    if (!handle) {
        fprintf(stderr, "Failed to create monitor\n");
        return 1;
    }

    // Start the monitor
    if (!rtlola_monitor_start(handle)) {
        fprintf(stderr, "Failed to start monitor\n");
        rtlola_monitor_free(handle);
        return 1;
    }

    // Example input values
    RTLolaInput inputs[2] = {
        { "input1", 2, { .float64_val = 3.14 } },
        { "input2", 0, { .uint64_val = 42 } }
    };

    // Process the inputs and get the verdict
    char* verdict = rtlola_process_inputs(handle, inputs, 2, 1.23);
    if (verdict != NULL) {
        printf("Verdict: %s\n", verdict);
        rtlola_free_string(verdict);
    } else {
        fprintf(stderr, "Error processing inputs\n");
    }

    // Free the monitor
    rtlola_monitor_free(handle);
    return 0;
}
```

## Memory Management

* The C functions that return strings (e.g., `rtlola_process_inputs`) return pointers to heap-allocated memory. It is important to free this memory using the `rtlola_free_string` function once you are done with the string.
* When the monitor is no longer needed, it should be freed using `rtlola_monitor_free`.

