use toml::Value;

use crate::CFA;
use crate::cfa::PlaneColor;
use crate::imgop::xyz::FlatColorMatrix;
use crate::imgop::xyz::Illuminant;

use std::collections::HashMap;

use super::BlackLevel;
use super::WhiteLevel;

/// Contains sanitized information about the raw image's properties
#[derive(Debug, Clone, Default)]
pub struct Camera {
  pub make: String,
  pub model: String,
  pub mode: String,
  pub clean_make: String,
  pub clean_model: String,
  pub remark: Option<String>,
  pub filesize: usize,
  pub raw_width: usize,
  pub raw_height: usize,
  //pub orientation: Orientation,
  pub whitelevel: Option<Vec<u32>>,
  pub blacklevel: Option<Vec<u32>>,
  pub blackareah: Option<(usize, usize)>,
  pub blackareav: Option<(usize, usize)>,
  pub xyz_to_cam: [[f32; 3]; 4],
  pub color_matrix: HashMap<Illuminant, FlatColorMatrix>,
  pub cfa: CFA,
  pub plane_color: PlaneColor,
  // Active area relative to sensor size
  pub active_area: Option<[usize; 4]>,
  // Recommended area relative to sensor size
  pub crop_area: Option<[usize; 4]>,
  // Hint/Replacement for EXIF BITDEPTH info
  pub bps: Option<usize>,
  // The BPS of the output after decoding
  pub real_bps: usize,
  pub highres_width: usize,
  pub default_scale: DefaultScale,
  pub best_quality_scale: BestQualityScale,
  pub hints: Vec<String>,
  pub params: HashMap<String, Value>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DefaultScale(pub [[u32; 2]; 2]);

impl Default for DefaultScale {
  fn default() -> Self {
    Self([[1, 1], [1, 1]])
  }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BestQualityScale(pub [u32; 2]);

impl Default for BestQualityScale {
  fn default() -> Self {
    Self([1, 1])
  }
}

impl Camera {
  pub fn find_hint(&self, hint: &str) -> bool {
    self.hints.contains(&hint.to_string())
  }

  pub fn param_usize(&self, name: &str) -> Option<usize> {
    self.params.get(name).and_then(|p| p.as_integer()).map(|i| i as usize)
  }

  pub fn param_i32(&self, name: &str) -> Option<i32> {
    self.params.get(name).and_then(|p| p.as_integer()).map(|i| i as i32)
  }

  pub fn param_str(&self, name: &str) -> Option<&str> {
    self.params.get(name).and_then(|p| p.as_str())
  }

  pub fn make_blacklevel(&self, cpp: usize) -> Option<BlackLevel> {
    self.blacklevel.as_ref().and_then(|x| {
      if x.len() == 1 {
        Some(BlackLevel::new(&vec![x[0]; cpp], 1, 1, cpp))
      } else if x.len() == self.cfa.width * self.cfa.height * cpp {
        Some(BlackLevel::new(x, self.cfa.width, self.cfa.height, cpp))
      } else {
        log::warn!("Camera config: invalid blacklevel data (len={}), ignoring", x.len());
        None
      }
    })
  }

  pub fn make_whitelevel(&self, cpp: usize) -> Option<WhiteLevel> {
    self.whitelevel.as_ref().and_then(|x| {
      if x.len() == 1 {
        Some(WhiteLevel(vec![x[0] as u32; cpp]))
      } else if x.len() == cpp {
        Some(WhiteLevel(x.clone()))
      } else {
        log::warn!("Camera config: invalid whitelevel data (len={}), ignoring", x.len());
        None
      }
    })
  }

  pub fn update_from_toml(&mut self, ct: &toml::value::Table) {
    for (name, val) in ct {
      match name.as_ref() {
        n @ "make" => {
          self.make = val.as_str().unwrap_or_else(|| { log::warn!("Camera TOML: {} must be a string, using empty", n); "" }).to_string();
        }
        n @ "model" => {
          self.model = val.as_str().unwrap_or_else(|| { log::warn!("Camera TOML: {} must be a string, using empty", n); "" }).to_string();
        }
        n @ "mode" => {
          self.mode = val.as_str().unwrap_or_else(|| { log::warn!("Camera TOML: {} must be a string, using empty", n); "" }).to_string();
        }
        n @ "clean_make" => {
          self.clean_make = val.as_str().unwrap_or_else(|| { log::warn!("Camera TOML: {} must be a string, using empty", n); "" }).to_string();
        }
        n @ "clean_model" => {
          self.clean_model = val.as_str().unwrap_or_else(|| { log::warn!("Camera TOML: {} must be a string, using empty", n); "" }).to_string();
        }
        n @ "remark" => {
          self.remark = Some(val.as_str().unwrap_or_else(|| { log::warn!("Camera TOML: {} must be a string, using empty", n); "" }).to_string());
        }
        n @ "whitepoint" => {
          let white = val.as_integer().unwrap_or_else(|| { log::warn!("Camera TOML: {} must be an integer, using 0", n); 0 }) as u32;
          self.whitelevel = Some(vec![white]);
        }
        n @ "blackpoint" => {
          let black = val.as_integer().unwrap_or_else(|| { log::warn!("Camera TOML: {} must be an integer, using 0", n); 0 }) as u32;
          self.blacklevel = Some(vec![black]);
        }
        n @ "blackareah" => {
          if let Some(vals) = val.as_array() {
            self.blackareah = Some((
              vals[0].as_integer().unwrap_or(0) as usize,
              vals[1].as_integer().unwrap_or(0) as usize,
            ));
          } else { log::warn!("Camera TOML: {} must be an array, skipping", n); }
        }
        n @ "blackareav" => {
          if let Some(vals) = val.as_array() {
            self.blackareav = Some((
              vals[0].as_integer().unwrap_or(0) as usize,
              vals[1].as_integer().unwrap_or(0) as usize,
            ));
          } else { log::warn!("Camera TOML: {} must be an array, skipping", n); }
        }
        "color_matrix" => {
          if let Some(color_matrix) = val.as_table() {
            for (illu_str, matrix) in color_matrix.into_iter() {
              let illu = Illuminant::new_from_str(illu_str).expect("invalid illuminant string in color_matrix");
              let xyz_to_cam = matrix
                .as_array()
                .expect("color matrix must be array")
                .iter()
                .map(|a| a.as_float().expect("color matrix values must be float") as f32)
                .collect();
              self.color_matrix.insert(illu, xyz_to_cam);
            }
          } else {
            eprintln!("Invalid matrix spec for {}", self.clean_model);
          }
          if self.color_matrix.is_empty() { log::warn!("Camera TOML: color_matrix is empty for {}", self.clean_model); }
        }
        n @ "active_area" => {
          if let Some(crop_vals) = val.as_array() {
            let mut crop = [0, 0, 0, 0];
            for (i, val) in crop_vals.iter().enumerate().take(4) {
              crop[i] = val.as_integer().unwrap_or(0) as usize;
            }
            self.active_area = Some(crop);
          } else { log::warn!("Camera TOML: {} must be an array, skipping", n); }
        }
        n @ "crop_area" => {
          if let Some(crop_vals) = val.as_array() {
            let mut crop = [0, 0, 0, 0];
            for (i, val) in crop_vals.iter().enumerate().take(4) {
              crop[i] = val.as_integer().unwrap_or(0) as usize;
            }
            self.crop_area = Some(crop);
          } else { log::warn!("Camera TOML: {} must be an array, skipping", n); }
        }
        n @ "color_pattern" => {
          self.cfa = CFA::new(val.as_str().unwrap_or_else(|| { log::warn!("Camera TOML: {} must be a string, using empty", n); "" }));
        }
        n @ "plane_color" => {
          self.plane_color = PlaneColor::new(val.as_str().unwrap_or_else(|| { log::warn!("Camera TOML: {} must be a string, using empty", n); "" }));
        }
        n @ "bps" => {
          self.bps = Some(val.as_integer().unwrap_or_else(|| { log::warn!("Camera TOML: {} must be an integer, using 0", n); 0 }) as usize);
        }
        n @ "real_bps" => {
          self.real_bps = val.as_integer().unwrap_or_else(|| { log::warn!("Camera TOML: {} must be an integer, using 0", n); 0 }) as usize;
        }
        n @ "filesize" => {
          self.filesize = val.as_integer().unwrap_or_else(|| { log::warn!("Camera TOML: {} must be an integer, using 0", n); 0 }) as usize;
        }
        n @ "raw_width" => {
          self.raw_width = val.as_integer().unwrap_or_else(|| { log::warn!("Camera TOML: {} must be an integer, using 0", n); 0 }) as usize;
        }
        n @ "raw_height" => {
          self.raw_height = val.as_integer().unwrap_or_else(|| { log::warn!("Camera TOML: {} must be an integer, using 0", n); 0 }) as usize;
        }
        n @ "highres_width" => {
          self.highres_width = val.as_integer().unwrap_or_else(|| { log::warn!("Camera TOML: {} must be an integer, using 0", n); 0 }) as usize;
        }
        n @ "default_scale" => {
          if let Some(scale_vals) = val.as_array() {
            if let (Some(scale_h), Some(scale_v)) = (scale_vals.get(0).and_then(|v| v.as_array()), scale_vals.get(1).and_then(|v| v.as_array())) {
              let scale = [
                [scale_h.get(0).and_then(|v| v.as_integer()).unwrap_or(1) as u32, scale_h.get(1).and_then(|v| v.as_integer()).unwrap_or(1) as u32],
                [scale_v.get(0).and_then(|v| v.as_integer()).unwrap_or(1) as u32, scale_v.get(1).and_then(|v| v.as_integer()).unwrap_or(1) as u32],
              ];
              self.default_scale = DefaultScale(scale);
            }
          } else { log::warn!("Camera TOML: {} must be an array, skipping", n); }
        }
        n @ "best_quality_scale" => {
          if let Some(scale_vals) = val.as_array() {
            self.best_quality_scale = BestQualityScale([
              scale_vals.get(0).and_then(|v| v.as_integer()).unwrap_or(1) as u32,
              scale_vals.get(1).and_then(|v| v.as_integer()).unwrap_or(1) as u32,
            ]);
          } else { log::warn!("Camera TOML: {} must be an array, skipping", n); }
        }
        n @ "hints" => {
          self.hints = Vec::new();
          if let Some(hints) = val.as_array() {
            for hint in hints {
              self.hints.push(hint.as_str().unwrap_or("").to_string());
            }
          } else { log::warn!("Camera TOML: {} must be an array, skipping", n); }
        }
        "params" => {
          if let Some(table) = val.as_table() {
          for (name, val) in table {
            self.params.insert(name.clone(), val.clone());
          }
          } // end if let Some(table)
        }
        "model_aliases" => {}
        "modes" => {} // ignore
        key => {
          log::warn!("Camera TOML: unknown key '{}', ignoring", key);
        }
      }
    }
  }

  pub fn new() -> Camera {
    Camera {
      make: "".to_string(),
      model: "".to_string(),
      mode: "".to_string(),
      clean_make: "".to_string(),
      clean_model: "".to_string(),
      remark: None,
      filesize: 0,
      raw_width: 0,
      raw_height: 0,
      whitelevel: None,
      blacklevel: None,
      blackareah: None,
      blackareav: None,
      xyz_to_cam: [[0.0; 3]; 4],
      color_matrix: HashMap::new(),
      cfa: CFA::new(""),
      plane_color: PlaneColor::default(),
      active_area: None,
      crop_area: None,
      bps: None,
      real_bps: 16,
      highres_width: usize::MAX,
      default_scale: DefaultScale::default(),
      best_quality_scale: BestQualityScale::default(),
      hints: Vec::new(),
      params: HashMap::new(),
      //orientation: Orientation::Unknown,
    }
  }
}
