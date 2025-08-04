Samsynk: A Home Automation System
=================================

Samsynk is a home automation system I'm building designed to work with the Sunsynk brand
of photovoltaic inverters. I'm building it for my own ability to control the inverter in
in my loft. It uses a usb-to-serial adaptor to talk modbus over rs323 or rs485 to the
inverter.

This allows me to:

* Read sensor values from the inverter, including the power draw, the battery SOC etc.
* Expose them to Prometheus for graphing etc.
* Change writeable sensor settings via an HTTP interface.
* Hopefully lots more in the future, including reading/writing to other data sources.

This works, but is still very much a WIP.
