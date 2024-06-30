use crate::sensor::{
    BasicSensor, CompoundSensor, FaultSensor, Sensor, SensorTypes, SerialSensor, TemperatureSensor,
};
use lazy_static::lazy_static;

pub const SERIAL: SerialSensor<'static> = SerialSensor {
    name: "Serial Sensor",
    registers: [3, 4, 5, 6, 7],
};

//pub const FAULTS: FaultSensor<'static> = FaultSensor {
//    name: "Sunsynk Fault Codes",
//    registers: [103, 104, 105, 106],
//};

lazy_static! {

    pub static ref FAULTS: FaultSensor<'static> = FaultSensor::new("Sunsynk Fault Codes", [103, 104, 105, 106]);

    pub static ref TEMP_SENSORS: [TemperatureSensor<'static>; 4] = [
        TemperatureSensor(Sensor::new("Battery Temperature", &[182], 10, false)),
        TemperatureSensor(Sensor::new("DC transformer temperature", &[90], 10, false)),
        TemperatureSensor(Sensor::new("Environment temperature", &[95], 10, false)),
        TemperatureSensor(Sensor::new("Radiator temperature", &[91], 10, false)),
    ];

    pub static ref COMPOUND_SENSORS: [CompoundSensor<'static>; 5] = [
        CompoundSensor::new("Essential Power", &[175, 169, 166], &[1, 1, -1], false, false),
        CompoundSensor::new("Essential 1 Power", &[175, 167, 166], &[1, 1, -1], false, false),
        CompoundSensor::new("Essential 2 Power", &[175, 169, 166], &[1, 1, -1], false, true),
        CompoundSensor::new("Non-Essential Power", &[172, 176], &[1, -1], true, false),
        CompoundSensor::new("Grid Current", &[160, 161], &[100, 100], false, false),
    ];

    pub static ref SENSORS: [BasicSensor<'static>; 52] = [
        // Battery
        BasicSensor(Sensor::new("Battery Voltage", &[183], 100, false)),
        BasicSensor(Sensor::new("Battery SOC", &[184], 1, false)),
        BasicSensor(Sensor::new("Battery Power", &[190], 1, true)),
        BasicSensor(Sensor::new("Battery Current", &[191], 100, true)),

        // Inverter
        BasicSensor(Sensor::new("Inverter Power", &[175], 1, true)),
        BasicSensor(Sensor::new("Inverter Voltage", &[154], 10, false)),
        BasicSensor(Sensor::new("Inverter Frequency", &[195], 100, false)),

        // Grid
        BasicSensor(Sensor::new("Grid Frequency", &[79], 100, false)),
        BasicSensor(Sensor::new("Grid Power", &[169], 1, true)),  // L1(167) + L2(168)
        BasicSensor(Sensor::new("Grid LD power", &[167], 1, true)),  // L1 seems to be LD
        BasicSensor(Sensor::new("Grid L2 power", &[168], 1, true)),
        BasicSensor(Sensor::new("Grid Voltage", &[150], 10, false)),
        BasicSensor(Sensor::new("Grid CT power", &[172], 1, true)),

        // Load
        BasicSensor(Sensor::new("Load Power", &[178], 1, true)),  // L1(176) + L2(177)
        BasicSensor(Sensor::new("Load L1 power", &[176], 1, true)),
        BasicSensor(Sensor::new("Load L2 power", &[177], 1, true)),

        // Solar
        BasicSensor(Sensor::new("PV1 Power", &[186], 1, true)),
        BasicSensor(Sensor::new("PV1 Voltage", &[109], 10, false)),
        BasicSensor(Sensor::new("PV1 Current", &[110], 10, false)),

        BasicSensor(Sensor::new("PV2 Power", &[187], 1, true)),
        BasicSensor(Sensor::new("PV2 Voltage", &[111], 10, false)),
        BasicSensor(Sensor::new("PV2 Current", &[112], 10, false)),

        BasicSensor(Sensor::new("AUX Power", &[166], 1, true)),

        // Energy
        BasicSensor(Sensor::new("Day Active Energy", &[60], 10, true)),
        BasicSensor(Sensor::new("Day Battery Charge", &[70], 10, false)),
        BasicSensor(Sensor::new("Day Battery discharge", &[71], 10, false)),
        BasicSensor(Sensor::new("Day Grid Export", &[77], 10, false)),
        BasicSensor(Sensor::new("Day Grid Import", &[76], 10, false)),
        BasicSensor(Sensor::new("Day Load Energy", &[84], 10, false)),
        BasicSensor(Sensor::new("Day PV Energy", &[108], 10, false)),
        BasicSensor(Sensor::new("Day Reactive Energy", &[61], 10, true)),
        BasicSensor(Sensor::new("Month Grid Energy", &[67], 10, false)),
        BasicSensor(Sensor::new("Month Load Energy", &[66], 10, false)),
        BasicSensor(Sensor::new("Month PV Energy", &[65], 10, false)),
        BasicSensor(Sensor::new("Total Active Energy", &[63, 64], 10, false)),  // signed?
        BasicSensor(Sensor::new("Total Battery Charge", &[72, 73], 10, false)),
        BasicSensor(Sensor::new("Total Battery Discharge", &[74, 75], 10, false)),
        BasicSensor(Sensor::new("Total Grid Export", &[81, 82], 10, false)),
        BasicSensor(Sensor::new("Total Grid Import", &[78, 80], 10, false)),
        BasicSensor(Sensor::new("Total Load Energy", &[85, 86], 10, false)),
        BasicSensor(Sensor::new("Total PV Energy", &[96, 97], 10, false)),
        BasicSensor(Sensor::new("Year Grid Export", &[98, 99], 10, false)),
        BasicSensor(Sensor::new("Year Load Energy", &[87, 88], 10, false)),
        BasicSensor(Sensor::new("Year PV Energy", &[68, 69], 10, false)),

        // General

        BasicSensor(Sensor::new("Grid Connected Status", &[194], 1, false)),

        // Settings
        BasicSensor(Sensor::new("Control Mode", &[200], 1, false)),
        BasicSensor(Sensor::new("Grid Charge Battery current", &[230], 1, false)),
        BasicSensor(Sensor::new("Grid Charge enabled", &[232], 1, true)),
        BasicSensor(Sensor::new("Battery charging voltage", &[312], -1, false)),
        BasicSensor(Sensor::new("Bat1 SOC", &[603], 1, false)),
        BasicSensor(Sensor::new("Bat1 Cycle", &[611], 1, false)),

        BasicSensor(Sensor::new_mut("Priority Mode", &[243], 1, false)),
    ];

    pub static ref ALL_SENSORS: Vec<SensorTypes<'static>> = vec![];
}
