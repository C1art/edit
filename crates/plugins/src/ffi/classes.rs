use vst3::ComPtr;
use vst3::Steinberg::kResultOk;
use vst3::Steinberg::{
    IPluginFactory2, IPluginFactory2Trait, IPluginFactoryTrait, PClassInfo2, PFactoryInfo,
};

use super::strings::from_char8;
use super::SharedModule;
use crate::ClassId;

pub(crate) struct ClassEntry {
    pub(crate) class_id: ClassId,
    pub(crate) name: String,
    pub(crate) vendor: String,
    pub(crate) version: String,
    pub(crate) category: String,
    pub(crate) subcategories: Vec<String>,
}

impl SharedModule {
    /// Classes from factories without `IPluginFactory2` are omitted: they cannot report
    /// subcategories, so effects cannot be told apart from instruments.
    pub(crate) fn classes(&self) -> Vec<ClassEntry> {
        let factory = self.factory();
        let Some(factory2) = factory.cast::<IPluginFactory2>() else {
            return Vec::new();
        };
        let factory_vendor = factory_vendor(factory);
        // SAFETY: plain method call on a live factory.
        let count = unsafe { factory.countClasses() };
        (0..count)
            .filter_map(|index| class_entry(&factory2, index, &factory_vendor))
            .collect()
    }
}

fn factory_vendor(factory: &ComPtr<vst3::Steinberg::IPluginFactory>) -> String {
    // SAFETY: PFactoryInfo is plain data for which all-zero bytes are valid.
    let mut info: PFactoryInfo = unsafe { std::mem::zeroed() };
    // SAFETY: `info` is a valid, writable PFactoryInfo.
    if unsafe { factory.getFactoryInfo(&mut info) } == kResultOk {
        from_char8(&info.vendor)
    } else {
        String::new()
    }
}

fn class_entry(
    factory: &ComPtr<IPluginFactory2>,
    index: i32,
    factory_vendor: &str,
) -> Option<ClassEntry> {
    // SAFETY: PClassInfo2 is plain data for which all-zero bytes are valid.
    let mut info: PClassInfo2 = unsafe { std::mem::zeroed() };
    // SAFETY: `info` is a valid, writable PClassInfo2 and `index < countClasses()`.
    if unsafe { factory.getClassInfo2(index, &mut info) } != kResultOk {
        return None;
    }
    let vendor = match from_char8(&info.vendor) {
        vendor if vendor.is_empty() => factory_vendor.to_owned(),
        vendor => vendor,
    };
    let subcategories = from_char8(&info.subCategories)
        .split('|')
        .filter(|subcategory| !subcategory.is_empty())
        .map(str::to_owned)
        .collect();
    Some(ClassEntry {
        class_id: ClassId(info.cid.map(|byte| byte as u8)),
        name: from_char8(&info.name),
        vendor,
        version: from_char8(&info.version),
        category: from_char8(&info.category),
        subcategories,
    })
}
