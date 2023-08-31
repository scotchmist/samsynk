use crate::sensor::{
    CompoundSensor, FaultSensor, PriorityMode, RWSensor, Sensor, SensorTypes, SerialSensor,
    TemperatureSensor,
};
use lazy_static::lazy_static;

lazy_static! {
    pub static ref SERIAL: SerialSensor<'static> = SerialSensor {
        name: "Serial Number",
        registers: [3, 4, 5, 6, 7],
    };

    pub static ref FAULTS: FaultSensor = FaultSensor::new("Sunsynk Fault Codes", [103, 104, 105, 106]);

    pub static ref TEMP_SENSORS: [TemperatureSensor<'static>; 4] = [
        TemperatureSensor(Sensor::new("Battery Temperature", &[182], 10, false)),
        TemperatureSensor(Sensor::new("DC transformer temperature", &[90], 10, false)),
        TemperatureSensor(Sensor::new("Environment temperature", &[95], 10, false)),
        TemperatureSensor(Sensor::new("Radiator temperature", &[91], 10, false)),
    ];

    pub static ref COMPOUND_SENSORS: [CompoundSensor<'static>; 3] = [
        CompoundSensor::new("Essential Power", &[175, 167, 166], &[1, 1, -1], false, false),
        CompoundSensor::new("Non-Essential Power", &[172, 176], &[1, -1], true, false),
        CompoundSensor::new("Grid current", &[160, 161], &[100, 100], false, false),
    ];

    pub static ref SENSORS: [Sensor<'static>; 51] = [
        // Battery
        Sensor::new("Battery Voltage", &[183], 100, false),
        Sensor::new("Battery SOC", &[184], 1, false),
        Sensor::new("Battery Power", &[190], 1, true),
        Sensor::new("Battery current", &[191], 100, true),

        // Inverter
        Sensor::new("Inverter power", &[175], 1, true),
        Sensor::new("Inverter voltage", &[154], 10, false),
        Sensor::new("Inverter frequency", &[195], 100, false),

        // Grid
        Sensor::new("Grid frequency", &[79], 100, false),
        Sensor::new("Grid power", &[169], 1, true),  // L1(167) + L2(168)
        Sensor::new("Grid LD power", &[167], 1, true),  // L1 seems to be LD
        Sensor::new("Grid L2 power", &[168], 1, true),
        Sensor::new("Grid voltage", &[150], 10, false),
        Sensor::new("Grid CT power", &[172], 1, true),

        // Load
        Sensor::new("Load power", &[178], 1, true),  // L1(176) + L2(177)
        Sensor::new("Load L1 power", &[176], 1, true),
        Sensor::new("Load L2 power", &[177], 1, true),

        // Solar
        Sensor::new("PV1 power", &[186], 1, true),
        Sensor::new("PV1 voltage", &[109], 10, false),
        Sensor::new("PV1 current", &[110], 10, false),

        Sensor::new("PV2 power", &[187], 1, true),
        Sensor::new("PV2 voltage", &[111], 10, false),
        Sensor::new("PV2 current", &[112], 10, false),

        // Power on Outputs
        Sensor::new("AUX power", &[166], 1, true),

        // Energy
        Sensor::new("Day Active Energy", &[60], 10, true),
        Sensor::new("Day Battery Charge", &[70], 10, false),
        Sensor::new("Day Battery discharge", &[71], 10, false),
        Sensor::new("Day Grid Export", &[77], 10, false),
        Sensor::new("Day Grid Import", &[76], 10, false),
        Sensor::new("Day Load Energy", &[84], 10, false),
        Sensor::new("Day PV Energy", &[108], 10, false),
        Sensor::new("Day Reactive Energy", &[61], 10, true),
        Sensor::new("Month Grid Energy", &[67], 10, false),
        Sensor::new("Month Load Energy", &[66], 10, false),
        Sensor::new("Month PV Energy", &[65], 10, false),
        Sensor::new("Total Active Energy", &[63, 64], 10, false),  // signed?
        Sensor::new("Total Battery Charge", &[72, 73], 10, false),
        Sensor::new("Total Battery Discharge", &[74, 75], 10, false),
        Sensor::new("Total Grid Export", &[81, 82], 10, false),
        Sensor::new("Total Grid Import", &[78, 80], 10, false),
        Sensor::new("Total Load Energy", &[85, 86], 10, false),
        Sensor::new("Total PV Energy", &[96, 97], 10, false),
        Sensor::new("Year Grid Export", &[98, 99], 10, false),
        Sensor::new("Year Load Energy", &[87, 88], 10, false),
        Sensor::new("Year PV Energy", &[68, 69], 10, false),

        // General

        Sensor::new("Grid Connected Status", &[194], 1, false),

        // Settings
        Sensor::new("Control Mode", &[200], 1, false),
        Sensor::new("Grid Charge Battery current", &[230], 1, false),
        Sensor::new("Grid Charge enabled", &[232], 1, true),
        Sensor::new("Battery charging voltage", &[312], -1, false),
        Sensor::new("Bat1 SOC", &[603], 1, false),
        Sensor::new("Bat1 Cycle", &[611], 1, false),
    ];

    pub static ref RWSENSORS: [RWSensor<'static, PriorityMode>; 1] = [
        RWSensor::new_mut("Priority Mode", &[243], PriorityMode::default())
    ];

    pub static ref ALL_SENSORS: Vec<SensorTypes<'static>> = vec![];
}
