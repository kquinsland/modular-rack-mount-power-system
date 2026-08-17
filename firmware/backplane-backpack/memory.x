/*
 * STM32C092FCP6 Rev A memory map.
 *
 * The final 8 KiB (four expected 2 KiB flash pages) is excluded from the
 * application image for power-fail-safe configuration and emergency-latch
 * records. The adapter compile-checks Embassy's 2 KiB erase and 8-byte write
 * geometry; confirm both against the reference manual and silicon during HIL.
 */
MEMORY
{
  FLASH  (rx)  : ORIGIN = 0x08000000, LENGTH = 248K
  CONFIG (rx)  : ORIGIN = 0x0803E000, LENGTH = 8K
  RAM    (rwx) : ORIGIN = 0x20000000, LENGTH = 30K
}

__config_start = ORIGIN(CONFIG);
__config_end = ORIGIN(CONFIG) + LENGTH(CONFIG);
