use metal::*;
use std::fs::File;
use std::mem;
use std::slice;
use indicatif::ProgressBar;
use indicatif::ProgressStyle;

mod gpu;
mod parser;

const START_YEAR: u16 = 0;
const START_MONTH: u16 = 0;
const START_DAY: u16 = 0;

const YEARS: u16 = 100;
const MONTHS: u16 = 100;
const DAYS: u16 = 100;

const TOTAL: usize = YEARS as usize * MONTHS as usize * DAYS as usize;


#[allow(dead_code)]
fn luhns(pin: [i32;10]) -> bool {
    //! Check a single PIN using the CPU
    //!
    //! This function is meant to be used to troubleshoot and test all other functions and methods
    //! Its essentialy the single source of truth that all other functions should follow
    let mut sum: i32 = 0;

    for (i, num) in pin.iter().enumerate() {
        sum += num + ((i as i32) & 1 ^ 1) * (num - ((num >= &5) as i32) * 9);
    }

    return sum % 10 == 0;
}

#[cfg(test)]
#[test]
fn luhns_check() {
    assert_eq!(luhns([0,6,1,0,0,9,2,4,5,4]), true);
    assert_eq!(luhns([0,6,0,3,1,7,9,2,7,6]), true);

    assert_eq!(luhns([1,6,0,3,1,7,9,2,7,6]), false);
}


fn main() {

    let mut offsets: [u16; 7] = [
        START_YEAR,
        START_MONTH,
        START_DAY,
        0,
        YEARS,
        MONTHS,
        DAYS
    ];

    let length = offsets.len() as u64;
    let size = length * core::mem::size_of::<u16>() as u64;
    println!("Validating {} pins in groups of {}", TOTAL*10_000, TOTAL);

    // Setup GPU
    println!("Setting up GPU...");
    let device = &gpu::get_device();
    let queue = device.new_command_queue();


    // Define thread count
    let grid_size = metal::MTLSize::new(
        YEARS as u64, //width
        MONTHS as u64, // height
        DAYS as u64); //depth


    // setup buffers
    println!("Creating buffers");
    let buffer_offsets = device.new_buffer_with_data(
        unsafe { mem::transmute(offsets.as_ptr()) }, // bytes
        size, // length
        MTLResourceOptions::StorageModeShared, // Storage mode
    );

    
    let buffer_result = device.new_buffer(
        (TOTAL * core::mem::size_of::<bool>()) as u64, // length
        MTLResourceOptions::StorageModeShared, // Storage mode
    );


    println!("Writing to output.txt");
    let mut out_file = File::create("output.txt").unwrap();

    let bar = ProgressBar::new(10_000);
    bar.set_style(ProgressStyle::with_template("[{elapsed_precise}] {bar:40.white/black} {pos:>7}/{len:7} {msg}").unwrap());
    for i in 0..10_000{
        bar.inc(1);

        // Update checksum
        offsets[3] = i;
        let a_ptr = buffer_offsets.contents() as *mut u16;
        unsafe { 
            let ptr = (a_ptr).offset(3);

            *ptr = i
        }

        let buffer = queue.new_command_buffer();
        let encoder = buffer.new_compute_command_encoder();

        // setup shader function
        gpu::use_function(&device, "check_pin", encoder);

        // init buffers
        encoder.set_buffer(0, Some(&buffer_offsets), 0);
        encoder.set_buffer(1, Some(&buffer_result), 0);

        // Compute
        bar.set_message("Computing");
        encoder.dispatch_threads(grid_size, gpu::max_group());
        encoder.end_encoding();
        buffer.commit();
        buffer.wait_until_completed();

        // results
        let ptr = buffer_result.contents() as *const bool;
        let len = buffer_result.length() as usize / mem::size_of::<bool>();
        let slice = unsafe { slice::from_raw_parts(ptr, len) };
        
        bar.set_message("Writing");
        parser::write(&mut out_file, &offsets, slice);
    }
    bar.finish();
}
