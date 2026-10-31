use crate::commands::utils::api_adapters::aspect_ratio::common_aspect_ratio::CommonAspectRatio;
use grok_consumer_client::endpoint_bindings::generate_image_websocket::messages::websocket_client_message::FastAspectRatio;

pub fn aspect_ratio_to_grok_image(aspect_ratio: CommonAspectRatio) -> FastAspectRatio {
  match aspect_ratio {
    // Exact
    CommonAspectRatio::Square => FastAspectRatio::Square,
    CommonAspectRatio::WideThreeByTwo => FastAspectRatio::WideThreeByTwo,
    CommonAspectRatio::TallTwoByThree => FastAspectRatio::TallTwoByThree,
    
    // Close enough
    CommonAspectRatio::SquareHd => FastAspectRatio::Square,

    // Non-matching
    CommonAspectRatio::Auto
    | CommonAspectRatio::Auto2k
    | CommonAspectRatio::Auto4k => FastAspectRatio::Square,
    
    // Mismatch - wide
    CommonAspectRatio::Wide 
    | CommonAspectRatio::WideFiveByFour 
    | CommonAspectRatio::WideFourByThree 
    | CommonAspectRatio::WideSixteenByNine 
    | CommonAspectRatio::WideTwentyOneByNine => FastAspectRatio::WideThreeByTwo,

    // Mismatch - tall
    CommonAspectRatio::Tall 
    | CommonAspectRatio::TallFourByFive 
    | CommonAspectRatio::TallThreeByFour 
    | CommonAspectRatio::TallNineBySixteen 
    | CommonAspectRatio::TallNineByTwentyOne => FastAspectRatio::TallTwoByThree,
  }
}
