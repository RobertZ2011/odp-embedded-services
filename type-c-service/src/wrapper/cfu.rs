//! CFU message bridge
//! TODO: remove this once we have a more generic FW update implementation
use embedded_services::fw_update::{FwUpdate as FwUpdateTrait, Error as FwError};
use embedded_services::cfu::component::*;
use embedded_cfu_protocol::protocol_definitions::*;
use embedded_services::type_c::controller::Controller;
use embedded_services::{debug, error};

use super::*;

impl<const N: usize, C: Controller + FwUpdateTrait> ControllerWrapper<'_, N, C> {
    /// Process a GetFwVersion command
    async fn process_get_fw_version(&self, target: &mut C) -> InternalResponseData {
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


        let dev_inf = FwVerComponentInfo::new(FwVersion::new(version), self.cfu_device.component_id());
        let comp_info: [FwVerComponentInfo; MAX_CMPT_COUNT] = [dev_inf; MAX_CMPT_COUNT];
        InternalResponseData::FwVersionResponse(GetFwVersionResponse {
                    header: GetFwVersionResponseHeader::new(
                        1,
                        GetFwVerRespHeaderByte3::NoSpecialFlags,
                    ),
                    component_info: comp_info,
                })
    }

    /// Process a CFU command
    pub async fn process_cfu_command(&self, controller: &mut C, command: &RequestData) -> Option<InternalResponseData> {
        match command {
            RequestData::FwVersionRequest => {
                debug!("Got FwVersionRequest");
                Some(self.process_get_fw_version(controller).await)
            }
            RequestData::GiveOffer(offer) => {
                debug!("Got GiveOffer");
                // accept any and all offers regardless of what version it is
                if offer.component_info.component_id == self.cfu_device.component_id() {
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
                    if let Err(_) = controller.start_fw_update().await {
                        error!("Failed to start FW update");
                        Some(InternalResponseData::ContentResponse(FwUpdateContentResponse::new(content.header.sequence_num, CfuUpdateContentResponseStatus::ErrorPrepare)))
                    } else {
                        if let Err(_) = controller.write_fw_contents(content.header.firmware_address as usize, data).await {
                            error!("Failed to write first block");
                            Some(InternalResponseData::ContentResponse(FwUpdateContentResponse::new(content.header.sequence_num, CfuUpdateContentResponseStatus::ErrorWrite)))
                        } else {
                            Some(InternalResponseData::ContentResponse(FwUpdateContentResponse::new(content.header.sequence_num, CfuUpdateContentResponseStatus::Success)))
                        }
                    }
               } else {
                    debug!("Got last block");
                     if let Err(_) = controller.write_fw_contents(content.header.firmware_address as usize, data).await {
                            error!("Failed to write first block");
                            Some(InternalResponseData::ContentResponse(FwUpdateContentResponse::new(content.header.sequence_num, CfuUpdateContentResponseStatus::ErrorWrite)))
                        } else {
                            Some(InternalResponseData::ContentResponse(FwUpdateContentResponse::new(content.header.sequence_num, CfuUpdateContentResponseStatus::Success)))
                        }
               }
            }
            RequestData::FinalizeUpdate => {
                debug!("Got FinalizeUpdate");
                if let Err(_) = controller.finalize_fw_update().await {
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

    /// Sends a CFU response to the command
    pub async fn send_cfu_response(&self, response: Option<InternalResponseData>) {
        self.cfu_device.send_response(response.unwrap_or(InternalResponseData::ComponentPrepared)).await;
    }
}