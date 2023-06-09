use crate::image::{Image, ImageDesc};
use crate::wgpu::math::Transform2D;
use crate::wgpu::wgpu_context::Texture;

struct ImageTexture {
    desc: ImageDesc,
    img: Option<Image>,
    tex: Option<Texture>,
    transform: Transform2D,
}