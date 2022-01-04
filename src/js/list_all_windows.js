global
    .get_window_actors()
    .map(a => a.meta_window.find_root_ancestor())
    .map(w => {
        let rect = w.get_frame_rect()

        return {
            window_class: w.wm_class,
            geom: {
                x: rect.x,
                y: rect.y,
                width: rect.width,
                height: rect.height,
                minimized: w.minimized
            },
            pid: w.get_pid(),
            stable_seq: w.get_stable_sequence(),
            gtk_app_id: w.get_gtk_application_id()
        }
    })
    .filter(data => data.geom.x >= 0 && data.geom.y >= 0)
