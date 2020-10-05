#[macro_use]
extern crate glium;
extern crate image;
extern crate twoway;
extern crate elementtree;

use std::env;
use std::fs::File;
use std::io::Read;
use glium::index::PrimitiveType;
use glium::{glutin, Surface};
use glium::uniforms::{ SamplerWrapFunction, MinifySamplerFilter, MagnifySamplerFilter };
use glutin::{event_loop::ControlFlow, event::Event, event::WindowEvent, event::MouseScrollDelta, event::ElementState::{Pressed, Released}};
use glutin::event::VirtualKeyCode::{Left, Right, Up, Down, PageUp, PageDown};

mod metadata;

fn main() -> Result<(), String> {
    let args = env::args().collect::<Vec<_>>();
    let image_name = args.get(1).ok_or(format!("Missing argument"))?;

    let input_img = image::open(image_name).unwrap().to_rgba();
    let mut buf = Vec::new();
    File::open(image_name).unwrap().take(1024*64).read_to_end(&mut buf).unwrap();
    let meta = metadata::parse(&buf[..], input_img.dimensions())?;
    println!("{:?}", meta);

    let event_loop = glutin::event_loop::EventLoop::new();
    let window = glutin::window::WindowBuilder::new();
    let context = glium::glutin::ContextBuilder::new();
    let display = glium::Display::new(window, context, &event_loop).unwrap();

    let image_dimensions = input_img.dimensions();
    let gl_image = glium::texture::RawImage2d::from_raw_rgba_reversed(&input_img.into_raw(), image_dimensions);
    let opengl_texture = glium::texture::SrgbTexture2d::new(&display, gl_image).unwrap();

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

    let mut yaw = 0.0f64;
    let mut yaw_rate = 0.0;
    let mut pitch = 0.0f64;
    let mut pitch_rate = 0.0;
    let mut zoom = 1.0f64;
    let mut zoom_rate = 1.0;
    let mut mouse_pos = (0.0f64, 0.0f64);
    let mut drag_state = None;

    event_loop.run(move |event, _, control_flow| {
        let (width, height) = display.get_framebuffer_dimensions();

        match event {
            Event::WindowEvent { event, .. } => match event {
                WindowEvent::CloseRequested => {
                    *control_flow = ControlFlow::Exit;
                    return;
                }
                WindowEvent::KeyboardInput { input, .. } => {
                    let speed = 1.0 / 4.0 / 60.0;

                    match (input.virtual_keycode, input.state) {
                        (Some(Left),  Pressed)  => yaw_rate = -speed,
                        (Some(Left),  Released) => yaw_rate = 0.0,
                        (Some(Right), Pressed)  => yaw_rate = speed,
                        (Some(Right), Released) => yaw_rate = 0.0,
                        (Some(Down),  Pressed)  => pitch_rate = -speed,
                        (Some(Down),  Released) => pitch_rate = 0.0,
                        (Some(Up),    Pressed)  => pitch_rate = speed,
                        (Some(Up),    Released) => pitch_rate = 0.0,
                        (Some(PageUp),Pressed)  => zoom_rate = 0.99,
                        (Some(PageUp),Released) => zoom_rate = 1.0,
                        (Some(PageDown),Pressed)  => zoom_rate = 1.01,
                        (Some(PageDown),Released) => zoom_rate = 1.0,
                        _ => {}
                    }
                }
                WindowEvent::MouseInput { state, .. } => {
                    match state {
                        Pressed => drag_state = Some((mouse_pos, (yaw, pitch))),
                        Released => drag_state = None
                    }
                }
                WindowEvent::Focused(false) => {
                    drag_state = None
                }
                WindowEvent::CursorMoved { position, .. } => {
                    mouse_pos = (
                        position.x / width as f64 * 2.0 - 1.0,
                        (position.y / height as f64 * 2.0 - 1.0) * (height as f64 / width as f64)
                    );

                    if let Some(((start_x, start_y), (start_yaw, start_pitch))) = drag_state {
                        let (x, y) = mouse_pos;
                        yaw = (start_x.atan() - x.atan()) / zoom + start_yaw;
                        pitch = (y.atan() - start_y.atan()) / zoom + start_pitch;
                    }

                    display.gl_window().window().request_redraw();
                }
                WindowEvent::MouseWheel { delta: MouseScrollDelta::LineDelta(_, y), .. } => {
                    zoom *= 1.0 + y as f64 * 0.08;
                    display.gl_window().window().request_redraw();
                }
                WindowEvent::MouseWheel { delta: MouseScrollDelta::PixelDelta(d), .. } => {
                    zoom *= 1.0 + d.y as f64 * 0.01;
                    display.gl_window().window().request_redraw();
                }
                _ => {}
            }
            Event::MainEventsCleared => {
                yaw += yaw_rate;
                pitch += pitch_rate;
                zoom *= zoom_rate;

                let idle = yaw_rate == 0.0 && pitch_rate == 0.0 && zoom_rate == 1.0;

                if !idle {
                    display.gl_window().window().request_redraw();
                }
            }
            Event::RedrawRequested(_) => {
                let mut target = display.draw();
                let (width, height) = target.get_dimensions();

                let uniforms = uniform! {
                    window_aspect_ratio: height as f32 / width as f32,
                    yaw: yaw as f32,
                    pitch: pitch as f32,
                    roll: 0.0f32,
                    zoom: zoom as f32,
                    image_offset: [ meta.crop_left, 1.0 - meta.crop_top - meta.height_ratio ],
                    image_fov: [ meta.width_ratio, meta.height_ratio ],
                    tex: opengl_texture.sampled()
                        .wrap_function(SamplerWrapFunction::Clamp)
                        .minify_filter(MinifySamplerFilter::Linear)
                        .magnify_filter(MagnifySamplerFilter::Linear)
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
            }
            _ => {}
        }
        
        *control_flow = ControlFlow::Wait;
    });
}
