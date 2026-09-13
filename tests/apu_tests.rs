use nes::apu::Apu;

#[test]
fn test_apu_pulse_length_counter() {
    let mut apu = Apu::new();

    // Enable Pulse 1
    apu.write_register(0x4015, 0x01);
    // Write length index 0 (table value: 10)
    apu.write_register(0x4003, 0x00);
    assert_eq!(apu.pulse1.length_counter.counter, 10);

    // Disable Pulse 1
    apu.write_register(0x4015, 0x00);
    assert_eq!(apu.pulse1.length_counter.counter, 0);
}

#[test]
fn test_apu_triangle_linear_counter() {
    let mut apu = Apu::new();

    // Enable Triangle
    apu.write_register(0x4015, 0x04);
    // Control flag clear, reload value 5
    apu.write_register(0x4008, 0x05);
    // Write timer high to set reload flag
    apu.write_register(0x400B, 0x00);

    // Step linear counter
    apu.triangle.step_linear_counter();
    assert_eq!(apu.triangle.linear_counter, 5);

    // Next step without control flag should decrement
    apu.triangle.step_linear_counter();
    assert_eq!(apu.triangle.linear_counter, 4);
}

#[test]
fn test_apu_noise_lfsr() {
    let mut apu = Apu::new();

    // Initial shift register should be 1
    assert_eq!(apu.noise.shift_register, 1);

    // Step timer
    apu.noise.timer_period = 0;
    apu.noise.step_timer();
    assert_ne!(apu.noise.shift_register, 1);
}

#[test]
fn test_apu_sample_generation_rate() {
    let mut apu = Apu::new();
    // Simulate 29780 CPU cycles (1 NTSC NES frame)
    for _ in 0..29780 {
        apu.step();
    }
    apu.flush_samples();

    let buf = apu.sample_buffer.lock().unwrap();
    // 29780 cycles * (44100 / 1789773) ~= 733.8 samples
    assert!(
        buf.len() >= 730 && buf.len() <= 738,
        "Expected ~734 samples per frame, got {}",
        buf.len()
    );
}
