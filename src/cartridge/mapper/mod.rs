use super::header::Mirroring;

pub trait Mapper: Send {
    fn read_prg(&self, addr: u16) -> u8;
    fn write_prg(&mut self, addr: u16, data: u8, cycle: u64);
    fn read_chr(&self, addr: u16) -> u8;
    fn write_chr(&mut self, addr: u16, data: u8);
    fn mirroring(&self) -> Mirroring;
    fn irq_state(&self) -> bool {
        false
    }
    fn step_scanline(&mut self) {}
    fn prg_ram(&self) -> &[u8] {
        &[]
    }
    fn prg_ram_mut(&mut self) -> &mut [u8] {
        &mut []
    }
}

pub mod mapper000;
pub mod mapper001;
pub mod mapper002;
pub mod mapper003;
pub mod mapper004;
pub mod mapper007;
