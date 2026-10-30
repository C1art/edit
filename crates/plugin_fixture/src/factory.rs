use std::ffi::c_void;
use std::ptr;

use vst3::Steinberg::PClassInfo_::ClassCardinality_::kManyInstances;
use vst3::Steinberg::PFactoryInfo_::FactoryFlags_::kNoFlags;
use vst3::Steinberg::{
    int32, kInvalidArgument, kResultOk, tresult, FIDString, FUnknown, IPluginFactory2,
    IPluginFactory2Trait, IPluginFactoryTrait, PClassInfo, PClassInfo2, PFactoryInfo, TUID,
};
use vst3::{uid, Class, ComWrapper};

use crate::combined::CombinedGain;
use crate::split::{SplitController, SplitProcessor};
use crate::strings::copy_char8;

const COMBINED: TUID = uid(0x5A1E_0001, 0x0000_4E44, 0x4954_4649, 0x5854_0001);
const SPLIT_PROCESSOR: TUID = uid(0x5A1E_0002, 0x0000_4E44, 0x4954_4649, 0x5854_0002);
const SPLIT_CONTROLLER: TUID = uid(0x5A1E_0003, 0x0000_4E44, 0x4954_4649, 0x5854_0003);
/// Listed but not creatable: it lets tests check that non-effects are filtered out.
const INSTRUMENT: TUID = uid(0x5A1E_0004, 0x0000_4E44, 0x4954_4649, 0x5854_0004);

const VENDOR: &str = "Edit Fixtures";

struct ClassDescription {
    id: TUID,
    category: &'static str,
    name: &'static str,
    subcategories: &'static str,
}

const CLASSES: [ClassDescription; 4] = [
    ClassDescription {
        id: COMBINED,
        category: "Audio Module Class",
        name: "Fixture Gain",
        subcategories: "Fx",
    },
    ClassDescription {
        id: SPLIT_PROCESSOR,
        category: "Audio Module Class",
        name: "Fixture Gain Split",
        subcategories: "Fx|Tools",
    },
    ClassDescription {
        id: SPLIT_CONTROLLER,
        category: "Component Controller Class",
        name: "Fixture Gain Split",
        subcategories: "",
    },
    ClassDescription {
        id: INSTRUMENT,
        category: "Audio Module Class",
        name: "Fixture Instrument",
        subcategories: "Instrument|Synth",
    },
];

pub(crate) struct Factory;

impl Class for Factory {
    type Interfaces = (IPluginFactory2,);
}

impl IPluginFactoryTrait for Factory {
    unsafe fn getFactoryInfo(&self, info: *mut PFactoryInfo) -> tresult {
        if info.is_null() {
            return kInvalidArgument;
        }
        // SAFETY: non-null out-pointer supplied by the host.
        let info = unsafe { &mut *info };
        copy_char8(VENDOR, &mut info.vendor);
        copy_char8("", &mut info.url);
        copy_char8("", &mut info.email);
        info.flags = kNoFlags as int32;
        kResultOk
    }

    unsafe fn countClasses(&self) -> int32 {
        CLASSES.len() as int32
    }

    unsafe fn getClassInfo(&self, index: int32, info: *mut PClassInfo) -> tresult {
        let class = usize::try_from(index).ok().and_then(|i| CLASSES.get(i));
        let Some(class) = class.filter(|_| !info.is_null()) else {
            return kInvalidArgument;
        };
        // SAFETY: non-null out-pointer supplied by the host.
        let info = unsafe { &mut *info };
        info.cid = class.id;
        info.cardinality = kManyInstances as int32;
        copy_char8(class.category, &mut info.category);
        copy_char8(class.name, &mut info.name);
        kResultOk
    }

    unsafe fn createInstance(
        &self,
        cid: FIDString,
        iid: FIDString,
        obj: *mut *mut c_void,
    ) -> tresult {
        if cid.is_null() || iid.is_null() || obj.is_null() {
            return kInvalidArgument;
        }
        // SAFETY: the host passes a 16-byte class ID.
        let cid = unsafe { *cid.cast::<TUID>() };
        let unknown = match cid {
            COMBINED => ComWrapper::new(CombinedGain::new()).to_com_ptr::<FUnknown>(),
            SPLIT_PROCESSOR => {
                ComWrapper::new(SplitProcessor::new(SPLIT_CONTROLLER)).to_com_ptr::<FUnknown>()
            }
            SPLIT_CONTROLLER => ComWrapper::new(SplitController::new()).to_com_ptr::<FUnknown>(),
            _ => None,
        };
        let Some(unknown) = unknown else {
            // SAFETY: non-null out-pointer supplied by the host.
            unsafe { *obj = ptr::null_mut() };
            return kInvalidArgument;
        };
        let object = unknown.as_ptr();
        // SAFETY: `object` is live; queryInterface adds the reference handed to the host,
        // and dropping `unknown` releases ours.
        unsafe { ((*(*object).vtbl).queryInterface)(object, iid.cast::<TUID>(), obj) }
    }
}

impl IPluginFactory2Trait for Factory {
    unsafe fn getClassInfo2(&self, index: int32, info: *mut PClassInfo2) -> tresult {
        let class = usize::try_from(index).ok().and_then(|i| CLASSES.get(i));
        let Some(class) = class.filter(|_| !info.is_null()) else {
            return kInvalidArgument;
        };
        // SAFETY: non-null out-pointer supplied by the host.
        let info = unsafe { &mut *info };
        info.cid = class.id;
        info.cardinality = kManyInstances as int32;
        copy_char8(class.category, &mut info.category);
        copy_char8(class.name, &mut info.name);
        info.classFlags = 0;
        copy_char8(class.subcategories, &mut info.subCategories);
        copy_char8(VENDOR, &mut info.vendor);
        copy_char8("1.0.0", &mut info.version);
        copy_char8("VST 3.7", &mut info.sdkVersion);
        kResultOk
    }
}
