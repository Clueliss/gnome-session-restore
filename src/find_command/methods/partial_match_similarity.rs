pub fn partial_match_similarity(search_term: &str, haystack: &str) -> f64 {
    let st_dot_split = search_term.split('.');

    let hs_dot_split = haystack.split('.');
    let hs_dash_split = haystack.split('-');

    let partial_dot_match = calculate_partial_fit_sum_similarity(st_dot_split.clone(), &hs_dot_split);
    let partial_mix_match_1 = calculate_partial_fit_sum_similarity(st_dot_split, &hs_dash_split);

    f64::max(partial_dot_match, partial_mix_match_1)
}

fn search_term_matching_similarity(
    search_term: &str,
    n_haystack_sections: usize,
    haystack_section_ix: usize,
    haystack_section: &str,
) -> f64 {
    const EMBED_SIM_WEIGHT_OFFSET: f64 = 0.3;
    const MATCH_FAIL_THRESHOLD: f64 = 0.6;
    const MATCH_FAIL_SEVERITY: f64 = 0.05;

    let n_hs_sections = n_haystack_sections as f64;
    let hs_pos = haystack_section_ix as f64 + 1.0;
    let hs_len = haystack_section.len() as f64;
    let st_len = search_term.len() as f64;

    let starts_with_sim = if haystack_section.starts_with(search_term) {
        (hs_len - (hs_len - st_len + 1.0).ln()) / hs_len
    } else {
        0.0
    };

    let str_sim = strsim::normalized_levenshtein(search_term, haystack_section);

    let sim = if starts_with_sim > 0.0 {
        (starts_with_sim * (1.0 + EMBED_SIM_WEIGHT_OFFSET) + str_sim * (1.0 - EMBED_SIM_WEIGHT_OFFSET)) / 2.0
    } else {
        str_sim
    };

    let length_correction_factor = 1.0 - (1.0 / (st_len + hs_len));
    let section_pos_correction_factor = (hs_pos / n_hs_sections).powi(2);

    let length_corrected_sim = sim * length_correction_factor;
    let fully_corrected_sim = length_corrected_sim * section_pos_correction_factor;

    if length_corrected_sim > MATCH_FAIL_THRESHOLD {
        fully_corrected_sim
    } else {
        -MATCH_FAIL_SEVERITY * (1.0 - fully_corrected_sim)
    }
}

fn calculate_partial_fit_sum_similarity<'a, 'b>(
    search_term_sections: impl Iterator<Item = &'a str>,
    haystack_sections: &(impl Iterator<Item = &'b str> + Clone),
) -> f64 {
    let n_hs_sections = haystack_sections.clone().count();

    let search_term_sections = {
        let mut tmp: Vec<_> = search_term_sections.collect();
        tmp.sort_unstable();
        tmp.dedup();

        tmp
    };

    let (count, sim_sum) = search_term_sections
        .into_iter()
        .map(|st_section| std::iter::repeat(st_section).zip(haystack_sections.clone().enumerate()))
        .flat_map(|pairs| {
            pairs
                .filter(|(st, (_, hs))| st.len() > 3 && hs.len() > 3)
                .map(|(st, (hs_ix, hs))| search_term_matching_similarity(st, n_hs_sections, hs_ix, hs))
        })
        .fold((0, 0.0), |(count, sum), sim| {
            if sim > 0.0 {
                (count + 1, sum + sim)
            } else {
                (count, sum + sim)
            }
        });

    if count > 0 {
        sim_sum / (count as f64)
    } else {
        0.0
    }
}

mod tests {
    #[test]
    fn test_pms() {
        dbg!(super::partial_match_similarity(
            "org.multimc.MultiMC",
            "net.lutris.multimc-2"
        ));
        dbg!(super::partial_match_similarity(
            "org.multimc.MultiMC",
            "org.gnome.multiply"
        ));
        dbg!(super::partial_match_similarity(
            "battle.net.exe",
            "net.lutris.battlenet-7"
        ));
        dbg!(super::partial_match_similarity("winemine.exe", "wine-winemine"));

        dbg!(super::partial_match_similarity("listen.tidal.com", "tidal"));
        dbg!(super::partial_match_similarity("Spotify", "tidal"));
        dbg!(super::partial_match_similarity("QjackCtl", "org.rncbc.qjackctl"));
        dbg!(super::partial_match_similarity("regedit.exe", "wine-regedit"));
    }
}
