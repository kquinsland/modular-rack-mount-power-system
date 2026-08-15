# Backplane Backpack

This board mates with the backplane - hench the 'backpack' moniker.
It shares the same mechanical footprint as the backplane and handles all of the smarts that would otherwise crowd out the high-current traces on the backplane.

At a very high level, it's a simple CAN ,<-> i2c bridge built around a STM32 micro.
There is a 3/4 pin header for a standard 12V pc fan and a simple neo pixel ring for status indication.
