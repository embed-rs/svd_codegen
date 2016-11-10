[![crates.io](https://img.shields.io/crates/v/svd_codegen.svg)](https://crates.io/crates/svd_codegen)
[![docs.rs](https://img.shields.io/badge/docs.rs-documentation-green.svg)](https://docs.rs/svd_codegen)

# `svd_codegen`

> Generate Rust register maps (`struct`s) from SVD files

This is a fork of [japaric/svd2rust](https://github.com/japaric/svd2rust) that generates a slightly different API.

## Usage

- Get the start address of each peripheral register block.

```
$ svd_codegen -i STM32F30x.svd
const GPIOA: usize = 0x48000000;
const GPIOB: usize = 0x48000400;
const GPIOC: usize = 0x48000800;
const GPIOD: usize = 0x48000c00;
const GPIOE: usize = 0x48001000;
const GPIOF: usize = 0x48001400;
(..)
```

- Generate a register map for a single peripheral.

```
$ svd_codegen -i STM32F30x.svd rcc | head
#[repr(C)]
/// Reset and clock control
pub struct Rcc {
    /// 0x00 - clock control register
    pub cr: ::volatile::ReadWrite<Cr>,
    /// 0x04 - PLL configuration register
    pub pllcfgr: ::volatile::ReadWrite<Pllcfgr>,
    /// 0x08 - clock configuration register
    pub cfgr: ::volatile::ReadWrite<Cfgr>,
    /// 0x0c - clock interrupt register
    pub cir: ::volatile::ReadWrite<Cir>,
    /// 0x10 - AHB1 peripheral reset register
    pub ahb1rstr: ::volatile::ReadWrite<Ahb1rstr>,
(..)
```

## API

The `svd_codegen` generates the following API for each peripheral:

### Register block

A register block "definition" as a `struct`. Example below:

``` rust
/// Inter-integrated circuit
#[repr(C)]
pub struct I2c1 {
    /// 0x00 - Control register 1
    pub cr1: ::volatile::ReadWrite<Cr1>,
    /// 0x04 - Control register 2
    pub cr2: ::volatile::ReadWrite<Cr2>,
    /// 0x08 - Own address register 1
    pub oar1: ::volatile::ReadWrite<Oar1>,
    /// 0x0c - Own address register 2
    pub oar2: ::volatile::ReadWrite<Oar2>,
    /// 0x10 - Timing register
    pub timingr: ::volatile::ReadWrite<Timingr>,
    /// 0x14 - Status register 1
    pub timeoutr: ::volatile::ReadWrite<Timeoutr>,
    /// 0x18 - Interrupt and Status register
    pub isr: ::volatile::ReadWrite<Isr>,
    /// 0x1c - Interrupt clear register
    pub icr: ::volatile::WriteOnly<Icr>,
    /// 0x20 - PEC register
    pub pecr: ::volatile::ReadOnly<Pecr>,
    /// 0x24 - Receive data register
    pub rxdr: ::volatile::ReadOnly<Rxdr>,
    /// 0x28 - Transmit data register
    pub txdr: ::volatile::ReadWrite<Txdr>,
}
```

The user has to "instantiate" this definition for each peripheral instance. They have several
choices:

- `static`s and/or `static mut`s. Example below:

``` rust
extern "C" {
    // I2C1 can be accessed in read-write mode
    pub static mut I2C1: I2c;
    // whereas I2C2 can only be accessed in "read-only" mode
    pub static I2C1: I2c;
}
```

Where the addresses of these register blocks must be provided by a linker script:

``` ld
/* layout.ld */
I2C1 = 0x40005400;
I2C2 = 0x40005800;
```

This has the side effect that the `I2C1` and `I2C2` symbols get "taken" so no other C/Rust symbol
(`static`, `function`, etc.) can have the same name.

- "constructor" functions. Example, equivalent to the `static` one, below:

``` rust
// Addresses of the register blocks. These are private.
const I2C1: usize = 0x40005400;
const I2C2: usize = 0x40005800;

// NOTE(unsafe) can alias references to mutable memory
pub unsafe fn i2c1() -> &'mut static I2C {
    unsafe { &mut *(I2C1 as *mut I2c) }
}

pub fn i2c2() -> &'static I2C {
    unsafe { &*(I2C2 as *const I2c) }
}
```

### `read` / `write` / `update`

Each register in the register block, e.g. the `cr1` field in the `I2c` struct, is wrapped in a volatile wrapper that exposes some methods:

- read-only registers only expose the `read` method.
- write-only registers only expose the `write` method.
- read-write registers exposes all the methods: `read`, `write`, and `update`.

The `read` method performs a single, volatile `LDR` instruction and the `write` method performs a single, volatile `STR` instruction. The update method takes a closure that modifies the register. It performs a `read`, passes the value to the closure and writes the modified value back:

```
pub fn update<F>(&mut self, f: F)
    where F: FnOnce(&mut T)
{
    let mut value = self.read();
    f(&mut value);
    self.write(value);
}
```

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or
  http://www.apache.org/licenses/LICENSE-2.0)
- MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in the
work by you, as defined in the Apache-2.0 license, shall be dual licensed as above, without any
additional terms or conditions.
