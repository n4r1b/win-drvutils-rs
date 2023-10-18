use crate::io::{file_obj::WduFileObject, irp::WduIrp};

pub struct WduCreate {
    // TODO
    // access_state: WduAccessState
    file_object: WduFileObject,
    desired_access: u32,
    file_attributes: u16,
    share_access: u16,
}

impl WduCreate {
    pub(crate) unsafe fn new(irp: &WduIrp) -> Self {
        let current_stack = WduIrp::current_stack(irp.as_ptr());
        let create_params = (*current_stack).Parameters.Create;

        let desired_access = if create_params.SecurityContext.is_null() {
            0
        } else {
            (*create_params.SecurityContext).DesiredAccess
        };

        WduCreate {
            desired_access,
            file_object: irp.original_file_object(),
            file_attributes: create_params.FileAttributes,
            share_access: create_params.ShareAccess,
        }
    }

    pub fn desired_access(&self) -> u32 {
        self.desired_access
    }

    pub fn file_attributes(&self) -> u16 {
        self.file_attributes
    }

    pub fn file_object(&self) -> &WduFileObject {
        &self.file_object
    }

    pub fn file_object_mut(&mut self) -> &mut WduFileObject {
        &mut self.file_object
    }
}
