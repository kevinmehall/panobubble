#[macro_use]
extern crate glium;
extern crate image;

use glium::index::PrimitiveType;
use glium::{glutin, Surface};
use glium::uniforms::SamplerWrapFunction;

fn main() {
    let image_name = "./8192.jpg";
    let cropped_area_top_pixels = 3190;
    let cropped_area_left_pixels = 0;
    let cropped_area_width_pixels = 18710;
    let cropped_area_height_pixels = 2961;
    let full_pano_height_pixels = 9354;
    let full_pano_width_pixels = 18710;

    let input_img = image::open(image_name).unwrap().to_rgba();

    let mut events_loop = glium::glutin::EventsLoop::new();
    let window = glium::glutin::WindowBuilder::new();
    let context = glium::glutin::ContextBuilder::new();
    let display = glium::Display::new(window, context, &events_loop).unwrap();

    let image_dimensions = input_img.dimensions();
    let gl_image = glium::texture::RawImage2d::from_raw_rgba_reversed(&input_img.into_raw(), image_dimensions);
    let opengl_texture = glium::texture::CompressedSrgbTexture2d::new(&display, gl_image).unwrap();

    let vertex_buffer = {
        #[derive(Copy, Clone)]
        struct Vertex {
            position: [f32; 2],
        }

        implement_vertex!(Vertex, position);

        glium::VertexBuffer::new(
            &display,
            &[
                Vertex { position: [-1.0, -1.0], },
                Vertex { position: [-1.0,  1.0], },
                Vertex { position: [1.0,   1.0], },
                Vertex { position: [1.0,  -1.0], },
            ],
        ).unwrap()
    };

    let index_buffer =
        glium::IndexBuffer::new(&display, PrimitiveType::TriangleStrip, &[1 as u16, 2, 0, 3])
            .unwrap();

    let program = program!(&display,
        140 => {
            vertex: "
                #version 140
                uniform mat4 matrix;
                in vec2 position;
                out vec2 v_tex_coords;
                void main() {
                    gl_Position = vec4(position, 0.0, 1.0);
                    v_tex_coords = position;
                }
            ",

            fragment: "
                #version 140

                const float PI = 3.14159265358979323846264;

                uniform float window_aspect_ratio;
                uniform float yaw;
                uniform float pitch;
                uniform float roll;
                uniform float zoom;

                uniform vec2 image_offset;
                uniform vec2 image_fov;

                uniform sampler2D tex;
                uniform sampler2D bgtex;
                in vec2 v_tex_coords;
                out vec4 f_color;
                void main() {
                    float x = v_tex_coords.x ;
                    float y = v_tex_coords.y * window_aspect_ratio;

                    float sinrot = sin(roll);
                    float cosrot = cos(roll);
                    float rot_x = x * cosrot - y * sinrot;
                    float rot_y = x * sinrot + y * cosrot;
                    float sintheta = sin(pitch);
                    float costheta = cos(pitch);
                    float a = zoom * costheta - rot_y * sintheta;
                    float root = sqrt(rot_x * rot_x + a * a);
                    float lambda = atan(rot_x / root, a / root) + yaw;
                    float phi = atan((rot_y * costheta + zoom * sintheta) / root);

                    lambda = mod(lambda + PI, PI * 2.0) - PI;

                    // Map texture to sphere
                    vec2 coord = vec2(0.5 + lambda / PI / 2, 0.5 + phi / PI);
                    vec2 pos = (coord - image_offset) / image_fov;

                    if (pos.y > 1 || pos.y < 0) {
                        f_color = vec4(0, 0, 0, 1);
                    } else {
                        f_color = texture(tex, pos);
                    }
                }
            "
        }
    ).unwrap();

    let mut closed = false;
    let mut yaw = 0.0f32;
    let mut yaw_rate = 0.0;
    let mut pitch = 0.0f32;
    let mut pitch_rate = 0.0;
    let mut zoom = 1.0f32;
    let mut zoom_rate = 1.0;
    while !closed {
        yaw += yaw_rate;
        pitch += pitch_rate;
        zoom *= zoom_rate;

        // drawing a frame
        let mut target = display.draw();
        let (width, height) = target.get_dimensions();

        let uniforms = uniform! {
            window_aspect_ratio: height as f32 / width as f32,
            yaw: yaw,
            pitch: pitch,
            roll: 0.0f32,
            zoom: zoom,
            image_offset: [
                cropped_area_left_pixels as f32 / full_pano_width_pixels as f32,
                cropped_area_top_pixels as f32 / full_pano_height_pixels as f32,
            ],
            image_fov: [
                cropped_area_width_pixels as f32 / full_pano_width_pixels as f32,
                cropped_area_height_pixels as f32 / full_pano_height_pixels as f32,
            ],
            tex: opengl_texture.sampled().wrap_function(SamplerWrapFunction::Clamp),
        };

        target.clear_color(0.0, 0.0, 0.0, 0.0);
        target
            .draw(
                &vertex_buffer,
                &index_buffer,
                &program,
                &uniforms,
                &Default::default(),
            )
            .unwrap();
        target.finish().unwrap();

        events_loop.poll_events(|ev| match ev {
            glutin::Event::WindowEvent { event, .. } => match event {
                glutin::WindowEvent::CloseRequested => closed = true,
                glutin::WindowEvent::KeyboardInput { input, .. } => {
                    use glutin::ElementState::{Pressed, Released};
                    use glutin::VirtualKeyCode::{Left, Right, Up, Down, PageUp, PageDown};

                    let speed = 1.0 / 4.0 / 60.0;

                    match (input.virtual_keycode, input.state) {
                        (Some(Left),  Pressed)  => yaw_rate = -speed,
                        (Some(Left),  Released) => yaw_rate = 0.0,
                        (Some(Right), Pressed)  => yaw_rate = speed,
                        (Some(Right), Released) => yaw_rate = 0.0,
                        (Some(Up),    Pressed)  => pitch_rate = -speed,
                        (Some(Up),    Released) => pitch_rate = 0.0,
                        (Some(Down),  Pressed)  => pitch_rate = speed,
                        (Some(Down),  Released) => pitch_rate = 0.0,
                        (Some(PageUp),Pressed)  => zoom_rate = 0.99,
                        (Some(PageUp),Released) => zoom_rate = 1.0,
                        (Some(PageDown),Pressed)  => zoom_rate = 1.01,
                        (Some(PageDown),Released) => zoom_rate = 1.0,
                        _ => {}
                    }
                }
                _ => (),
            },
            _ => (),
        });
    }
}
