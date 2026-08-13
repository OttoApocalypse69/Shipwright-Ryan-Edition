#[derive(Debug)]
pub(crate) struct ProbeResult {
    pub(crate) available: bool,
    pub(crate) summary: String,
    pub(crate) details: String,
}

#[cfg(target_os = "windows")]
pub(crate) fn probe_controller() -> ProbeResult {
    use windows::Win32::UI::Input::XboxController::{XINPUT_STATE, XInputGetState};

    let connected = (0..4).find(|index| {
        let mut state = XINPUT_STATE::default();
        unsafe { XInputGetState(*index, &mut state) == 0 }
    });

    match connected {
        Some(index) => ProbeResult {
            available: true,
            summary: format!("Controller {} detected", index + 1),
            details: "An XInput-compatible controller responded to the native Windows probe.".to_owned(),
        },
        None => ProbeResult {
            available: false,
            summary: "No XInput controller detected".to_owned(),
            details: "Keyboard controls remain available. SDL-specific controller enumeration is confirmed again by the Zelda runtime."
                .to_owned(),
        },
    }
}

#[cfg(not(target_os = "windows"))]
pub(crate) fn probe_controller() -> ProbeResult {
    ProbeResult {
        available: false,
        summary: "Controller probe unavailable on this platform".to_owned(),
        details: "Windows is the first-class Milestone 1.5 target.".to_owned(),
    }
}

#[cfg(target_os = "windows")]
pub(crate) fn probe_graphics() -> ProbeResult {
    use windows::Win32::Foundation::HMODULE;
    use windows::Win32::Graphics::Direct3D::{
        D3D_DRIVER_TYPE_HARDWARE, D3D_FEATURE_LEVEL, D3D_FEATURE_LEVEL_10_0,
        D3D_FEATURE_LEVEL_10_1, D3D_FEATURE_LEVEL_11_0, D3D_FEATURE_LEVEL_11_1,
    };
    use windows::Win32::Graphics::Direct3D11::{
        D3D11_CREATE_DEVICE_BGRA_SUPPORT, D3D11_SDK_VERSION, D3D11CreateDevice, ID3D11Device,
        ID3D11DeviceContext,
    };
    use windows::Win32::Graphics::Dxgi::IDXGIAdapter;

    let levels = [
        D3D_FEATURE_LEVEL_11_1,
        D3D_FEATURE_LEVEL_11_0,
        D3D_FEATURE_LEVEL_10_1,
        D3D_FEATURE_LEVEL_10_0,
    ];
    let mut selected = D3D_FEATURE_LEVEL::default();
    let mut device: Option<ID3D11Device> = None;
    let mut context: Option<ID3D11DeviceContext> = None;
    let result = unsafe {
        D3D11CreateDevice(
            None::<&IDXGIAdapter>,
            D3D_DRIVER_TYPE_HARDWARE,
            HMODULE::default(),
            D3D11_CREATE_DEVICE_BGRA_SUPPORT,
            Some(&levels),
            D3D11_SDK_VERSION,
            Some(&mut device),
            Some(&mut selected),
            Some(&mut context),
        )
    };

    match result {
        Ok(()) => ProbeResult {
            available: true,
            summary: format!("Direct3D feature level 0x{:x} available", selected.0),
            details: "A hardware Direct3D 11 device was created and released successfully."
                .to_owned(),
        },
        Err(error) => ProbeResult {
            available: false,
            summary: "Direct3D hardware device unavailable".to_owned(),
            details: format!("Native Direct3D probe failed: {error}"),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_probes_return_actionable_results() {
        let graphics = probe_graphics();
        let controller = probe_controller();
        assert!(!graphics.summary.is_empty());
        assert!(!graphics.details.is_empty());
        assert!(!controller.summary.is_empty());
        assert!(!controller.details.is_empty());
        eprintln!("graphics: {} — {}", graphics.summary, graphics.details);
        eprintln!(
            "controller: {} — {}",
            controller.summary, controller.details
        );
    }
}

#[cfg(not(target_os = "windows"))]
pub(crate) fn probe_graphics() -> ProbeResult {
    ProbeResult {
        available: false,
        summary: "Graphics probe unavailable on this platform".to_owned(),
        details: "Windows is the first-class Milestone 1.5 target.".to_owned(),
    }
}
