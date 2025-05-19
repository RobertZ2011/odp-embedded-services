use crate::fw_update::{FwUpdate, Error as FwError};
use crate::cfu::{CfuDevice, RequestData, InternalResponseData, CfuError, CfuDeviceContainer};
use core::cell::RefCell;
use embedded_cfu_protocol::protocol_definitions::*;
use crate::{debug, error};

pub struct MessageBridge<T: FwUpdate> {
    target: RefCell<T>,
    device: CfuDevice,
}

impl<T: FwUpdate> MessageBridge<T> {
    pub fn new(device: CfuDevice, target: T) -> Self {
        Self { target: RefCell::new(target), device }
    }
}

impl<T: FwUpdate> MessageBridge<T> {
    pub async fn wait_command(&self) -> RequestData {
        self.device.wait_request().await
    }

    async fn process_get_fw_version(&self, target: &mut T) -> InternalResponseData {
        let version = match target.get_active_fw_version().await {
            Ok(v) => v,
            Err(FwError::General(e)) => {
                error!("Failed to get active firmware version: {:?}", e);
                return InternalResponseData::ComponentBusy;
            }
            Err(FwError::Bus(_)) => {
                error!("Failed to get active firmware version, bus error");
                return InternalResponseData::ComponentBusy;
            }
        };


        let dev_inf = FwVerComponentInfo::new(FwVersion::new(version), self.device.component_id());
        let mut comp_info: [FwVerComponentInfo; MAX_CMPT_COUNT] = [dev_inf; MAX_CMPT_COUNT];
        InternalResponseData::FwVersionResponse(GetFwVersionResponse {
                    header: GetFwVersionResponseHeader::new(
                        1,
                        GetFwVerRespHeaderByte3::NoSpecialFlags,
                    ),
                    component_info: comp_info,
                })
    }

    pub async fn process_command(&self, command: RequestData) -> Option<InternalResponseData> {
        let mut target = self.target.borrow_mut();
        match command {
            RequestData::FwVersionRequest => {
                debug!("Got FwVersionRequest");
                Some(self.process_get_fw_version(&mut target).await)
            }
            RequestData::GiveOffer(offer) => {
                debug!("Got GiveOffer");
                // accept any and all offers regardless of what version it is
                if offer.component_info.component_id == self.device.component_id() {
                    debug!("Accepting offer");
                    Some(InternalResponseData::OfferResponse(FwUpdateOfferResponse::new_accept(HostToken::Driver)))
                } else {
                    debug!("Rejecting offer, ID mimismatch");
                    None
                }
            }
            RequestData::GiveContent(content) => {
               let data = &content.data[0..content.header.data_length as usize];
               if content.header.flags & FW_UPDATE_FLAG_FIRST_BLOCK != 0 {
                    debug!("Got first block");
                    // Need to start the update
                    if let Err(_) = target.start_fw_update().await {
                        error!("Failed to start FW update");
                        Some(InternalResponseData::ContentResponse(FwUpdateContentResponse::new(content.header.sequence_num, CfuUpdateContentResponseStatus::ErrorPrepare)))
                    } else {
                        if let Err(_) = target.write_fw_contents(content.header.firmware_address as usize, data).await {
                            error!("Failed to write first block");
                            Some(InternalResponseData::ContentResponse(FwUpdateContentResponse::new(content.header.sequence_num, CfuUpdateContentResponseStatus::ErrorWrite)))
                        } else {
                            Some(InternalResponseData::ContentResponse(FwUpdateContentResponse::new(content.header.sequence_num, CfuUpdateContentResponseStatus::Success)))
                        }
                    }
               } else {
                    debug!("Got last block");
                     if let Err(_) = target.write_fw_contents(content.header.firmware_address as usize, data).await {
                            error!("Failed to write first block");
                            Some(InternalResponseData::ContentResponse(FwUpdateContentResponse::new(content.header.sequence_num, CfuUpdateContentResponseStatus::ErrorWrite)))
                        } else {
                            Some(InternalResponseData::ContentResponse(FwUpdateContentResponse::new(content.header.sequence_num, CfuUpdateContentResponseStatus::Success)))
                        }
               }
            }
            RequestData::FinalizeUpdate => {
                debug!("Got FinalizeUpdate");
                if let Err(_) = target.finalize_fw_update().await {
                    error!("Failed to finalize FW update");
                }
                None
            },
            _ => {
                debug!("Got other command: {:#?}", command);
                None

            }
        }
    }

    pub async fn send_response(&self, response: InternalResponseData) {
        self.device.send_response(response).await;
    }

    pub async fn process(&self) {
            let command = self.wait_command().await;
            if let Some(response) = self.process_command(command).await {
                self.send_response(response).await;
            } else {
                self.send_response(InternalResponseData::ComponentPrepared).await;
            }
            
    }
}

impl<T: FwUpdate> CfuDeviceContainer for MessageBridge<T> {
    fn get_cfu_component_device(&self) -> &CfuDevice {
        &self.device
    }
}

impl<Inner> From<FwError<Inner>> for CfuError
{
    fn from(_err: FwError<Inner>) -> Self {
        CfuError::BadImage
    }
}