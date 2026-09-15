MEMORY
{
  /* STM32C092xC: 256 KiB flash, 30 KiB SRAM. */
  FLASH                 : ORIGIN = 0x08006000, LENGTH = 110K
  RAM                   : ORIGIN = 0x20000000, LENGTH = 30K
  BOOTLOADER_STATE      : ORIGIN = 0x08004000, LENGTH = 8K
  ACTIVE                : ORIGIN = 0x08006000, LENGTH = 110K
  DFU                   : ORIGIN = 0x08021800, LENGTH = 112K
  CONFIG                : ORIGIN = 0x0803E000, LENGTH = 8K
}

__bootloader_state_start = ORIGIN(BOOTLOADER_STATE);
__bootloader_state_end = ORIGIN(BOOTLOADER_STATE) + LENGTH(BOOTLOADER_STATE);
__bootloader_active_start = ORIGIN(ACTIVE);
__bootloader_active_end = ORIGIN(ACTIVE) + LENGTH(ACTIVE);
__bootloader_dfu_start = ORIGIN(DFU);
__bootloader_dfu_end = ORIGIN(DFU) + LENGTH(DFU);
