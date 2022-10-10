use std::str::FromStr;
use memchr::memmem;
use elementtree::Element;

#[derive(Copy, Clone, Debug)]
pub struct PanoMeta {
    pub width_ratio: f32,
    pub height_ratio: f32,
    pub crop_left: f32,
    pub crop_top: f32,
}

/// Extract panorama metadata from the image
///
/// Specifically, an equirectangular image may cover less than the full 360x180 FOV
/// so we need to know where to place it on the sphere.
pub fn parse(buf: &[u8], (w, h): (u32, u32)) -> Result<PanoMeta, String> {
    let gpano_result = find_xmp(buf).and_then(parse_gpano);

    if gpano_result.is_err() && w/2 == h {
        // Assume it's a full 360x180 degree image
        Ok(PanoMeta {
            width_ratio: 1.0,
            height_ratio: 1.0,
            crop_left: 0.0,
            crop_top: 0.0,
        })
    } else {
        gpano_result
    }
}

fn find_xmp(buf: &[u8]) -> Result<Element, String> {
    // Almost like actually parsing the image headers...
    let start_pattern = b"<x:xmpmeta";
    let end_pattern = b"</x:xmpmeta>";
    if let Some(start) = memmem::find(buf, start_pattern) {
        if let Some(end) = memmem::find(&buf[start..], end_pattern) {
            let xmp_str = &buf[start..start+end+end_pattern.len()];
            return Element::from_reader(xmp_str).map_err(|e| format!("Failed to parse XMP: {:?}", e));
        }
    }
    return Err(format!("No XMP found in image"));
}

/// Find GPano XMP tags
/// https://developers.google.com/streetview/spherical-metadata
fn parse_gpano(root: Element) -> Result<PanoMeta, String> {
    const RDF: &'static str = "http://www.w3.org/1999/02/22-rdf-syntax-ns#";
    const GPANO: &'static str = "http://ns.google.com/photos/1.0/panorama/";

    let elem = root.find((RDF, "RDF")).and_then(|r| {
        r.find_all((RDF, "Description"))
        .find(|elem| elem.find((GPANO, "UsePanoramaViewer")).is_some() || elem.get_attr((GPANO, "UsePanoramaViewer")).is_some())
    });

    let elem = elem.ok_or(format!("No GPano Description tag"))?;

    // Some implementers (e.g. Android camera) put the fields in attributes of the Description,
    // while some (e.g. Hugin) put them in child tags, as specified by the link abouve.
    // We'll look in both places.
    fn field<T:FromStr>(e: &Element, tag: &str) -> Result<T, String> {
        e.find((GPANO, tag))
            .map(|c| c.text())
            .or_else(|| e.get_attr((GPANO, tag)))
            .and_then(|v| v.trim().parse::<T>().ok())
            .ok_or_else(|| format!("Missing GPano:{}", tag))
    }

    let projection_type = field::<String>(elem, "ProjectionType")?;
    if projection_type != "equirectangular" {
        return Err(format!("Unsupported projection type {}", projection_type));
    }

    let cropped_width   = field::<u32>(elem, "CroppedAreaImageWidthPixels")?;
    let cropped_height  = field::<u32>(elem, "CroppedAreaImageHeightPixels")?;
    let full_width      = field::<u32>(elem, "FullPanoWidthPixels")?;
    let full_height     = field::<u32>(elem, "FullPanoHeightPixels")?;
    let cropped_left    = field::<u32>(elem, "CroppedAreaLeftPixels")?;
    let cropped_top     = field::<u32>(elem, "CroppedAreaTopPixels")?;

    println!("GPano: {} {} {} {} {} {} {}", projection_type, cropped_width, cropped_height, full_width, full_height, cropped_left, cropped_top);

    Ok(PanoMeta {
        width_ratio: cropped_width as f32 / full_width as f32,
        height_ratio: cropped_height as f32 / full_height as f32,
        crop_left: cropped_left as f32 / full_width as f32,
        crop_top: cropped_top as f32 / full_height as f32,
    })
}
