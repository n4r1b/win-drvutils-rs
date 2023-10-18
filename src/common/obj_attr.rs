use crate::{inner_getters_value, strings::unicode::string::WduUnicodeString};
use windows_sys::{
    Wdk::Foundation::OBJECT_ATTRIBUTES,
    Win32::{
        Foundation::HANDLE,
        System::Kernel::{
            OBJ_CASE_INSENSITIVE, OBJ_DONT_REPARSE, OBJ_EXCLUSIVE, OBJ_FORCE_ACCESS_CHECK,
            OBJ_IGNORE_IMPERSONATED_DEVICEMAP, OBJ_INHERIT, OBJ_KERNEL_HANDLE, OBJ_OPENIF,
            OBJ_PERMANENT, OBJ_VALID_ATTRIBUTES,
        },
    },
};

use bitflags::bitflags;

bitflags! {
    #[derive(Debug, Clone, Copy)]
    pub struct WduObjHandleAttributes : i32 {
        const Inherit = OBJ_INHERIT;
        const Permanent = OBJ_PERMANENT;
        const Exclusive = OBJ_EXCLUSIVE;
        const CaseInsensitive = OBJ_CASE_INSENSITIVE;
        const OpenIf = OBJ_OPENIF;
        const KernelHandle = OBJ_KERNEL_HANDLE;
        const ForceAccessCheck = OBJ_FORCE_ACCESS_CHECK;
        const DontReparse = OBJ_DONT_REPARSE;
        const IgnoreImpersonatedDeviceMap = OBJ_IGNORE_IMPERSONATED_DEVICEMAP;
        const ValidAttributes = OBJ_VALID_ATTRIBUTES;
    }
}

pub struct WduObjectAttributes {
    obj_attr: OBJECT_ATTRIBUTES,
}

impl Default for WduObjectAttributes {
    fn default() -> Self {
        let obj_attr = OBJECT_ATTRIBUTES {
            Length: 0,
            RootDirectory: 0,
            ObjectName: core::ptr::null(),
            Attributes: 0,
            SecurityDescriptor: core::ptr::null(),
            SecurityQualityOfService: core::ptr::null(),
        };

        Self { obj_attr }
    }
}

inner_getters_value!(WduObjectAttributes, obj_attr, OBJECT_ATTRIBUTES);

impl WduObjectAttributes {
    pub fn root_dir(mut self, handle: HANDLE) -> Self {
        self.obj_attr.RootDirectory = handle;
        self
    }

    pub fn object_name(mut self, obj_name: WduUnicodeString) -> Self {
        self.obj_attr.ObjectName = obj_name.as_ptr();
        self
    }

    pub fn attributes(mut self, attributes: WduObjHandleAttributes) -> Self {
        // Attribute values declared as i32. ObjectAttributes.Attributes declared as u32
        self.obj_attr.Attributes = attributes.bits() as u32;
        self
    }

    pub fn security_descriptor(self) -> Self {
        todo!()
    }

    pub fn build(mut self) -> Self {
        self.obj_attr.Length = core::mem::size_of::<OBJECT_ATTRIBUTES>() as u32;
        self
    }
}
