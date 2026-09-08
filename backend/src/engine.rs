use crate::{
    demo::catalog,
    genre::{canonicalize_genre, exploration_genres, genre_key, is_unknown_genre},
    identity::{artist_identity_keys, normalized_track_key, same_recording},
    models::{
        BridgeTrack, ComparisonReport, PersonalDemo, Playlist, Recommendation,
        RecommendationSummary, RecommendationZoneSummary, RouteStep, TasteMetric, TasteReport,
        Track,
    },
    normalize::normalize_text,
};
use std::collections::{HashMap, HashSet};
use uuid::Uuid;

pub fn parse_manual_playlist(name: &str, text: &str) -> Result<Playlist, String> {
    let tracks: Vec<Track> = text
        .lines()
        .filter_map(|raw| {
            let line = raw.trim().trim_start_matches(|c: char| {
                c.is_ascii_digit() || matches!(c, '.' | ')' | '-' | '•')
            });
            if line.is_empty() {
                return None;
            }
            let (artist, title) = line
                .split_once(" - ")
                .or_else(|| line.split_once(" – "))
                .or_else(|| line.split_once(" — "))?;
            let title = title.trim();
            let artist = artist.trim();
            if title.is_empty() || artist.is_empty() {
                return None;
            }
            Some(Track {
                id: Uuid::new_v4().to_string(),
                title: title.to_string(),
                normalized_title: normalize_text(title),
                artists: vec![artist.to_string()],
                album: None,
                genres: vec!["待补全".into()],
                release_year: None,
                language: None,
                duration_ms: None,
                platform: "manual".into(),
                platform_url: None,
                external_ids: HashMap::new(),
                version_type: crate::normalize::detect_version(title),
                mood_tags: vec![],
                energy_score: None,
                popularity: None,
                metadata_confidence: 0.45,
            })
        })
        .collect();

    if tracks.is_empty() {
        return Err("未识别到歌曲；请每行使用“歌手 - 歌名”格式。".into());
    }

    Ok(Playlist {
        id: Uuid::new_v4().to_string(),
        name: name.to_string(),
        owner_label: "当前用户".into(),
        source: "手动文本".into(),
        is_demo: false,
        tracks,
    })
}

pub fn analyze_playlist(playlist: &Playlist) -> TasteReport {
    let mut genres: HashMap<String, usize> = HashMap::new();
    let mut artists: HashMap<String, usize> = HashMap::new();
    let mut eras: HashMap<String, usize> = HashMap::new();
    let mut albums: HashMap<String, usize> = HashMap::new();
    let mut track_keys: HashMap<String, usize> = HashMap::new();
    let mut energies = Vec::new();
    for track in &playlist.tracks {
        for genre in track.genres.iter().filter(|genre| !is_unknown_genre(genre)) {
            *genres.entry(canonicalize_genre(genre)).or_default() += 1;
        }
        for artist in &track.artists {
            *artists.entry(artist.clone()).or_default() += 1;
        }
        if let Some(year) = track.release_year {
            let bucket = format!("{}s", year / 10 * 10);
            *eras.entry(bucket).or_default() += 1;
        }
        if let Some(album) = track
            .album
            .as_ref()
            .filter(|album| !album.trim().is_empty())
        {
            *albums.entry(album.clone()).or_default() += 1;
        }
        *track_keys
            .entry(format!(
                "{}|{}",
                normalize_text(&track.title),
                normalize_text(&track.artists.join(" "))
            ))
            .or_default() += 1;
        if let Some(energy) = track.energy_score {
            energies.push(energy);
        }
    }
    let sorted = |map: HashMap<String, usize>| {
        let mut values: Vec<_> = map.into_iter().collect();
        values.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
        values
    };
    let genre_distribution = sorted(genres);
    let artist_distribution = sorted(artists);
    let era_distribution = sorted(eras);
    let album_distribution = sorted(albums);
    let track_count = playlist.tracks.len();
    let genre_matched_count = playlist
        .tracks
        .iter()
        .filter(|track| track.genres.iter().any(|genre| !is_unknown_genre(genre)))
        .count();
    let genre_coverage = if track_count == 0 {
        0.0
    } else {
        genre_matched_count as f32 / track_count as f32
    };
    let energy_matched_count = energies.len();
    let energy_coverage = if track_count == 0 {
        0.0
    } else {
        energy_matched_count as f32 / track_count as f32
    };
    let duplicate_track_count = track_keys
        .values()
        .map(|count| count.saturating_sub(1))
        .sum();
    let collaboration_track_count = playlist
        .tracks
        .iter()
        .filter(|track| track.artists.len() > 1)
        .count();
    let genre_diversity = if track_count == 0 {
        0.0
    } else {
        (genre_distribution.len() as f32 / track_count as f32).min(1.0)
    };
    let top_artist_count = artist_distribution.first().map(|v| v.1).unwrap_or(0);
    let artist_concentration = if track_count == 0 {
        0.0
    } else {
        top_artist_count as f32 / track_count as f32
    };
    let average_energy = if energies.is_empty() {
        0.0
    } else {
        energies.iter().sum::<f32>() / energies.len() as f32
    };
    let confidence = playlist
        .tracks
        .iter()
        .map(|t| t.metadata_confidence)
        .sum::<f32>()
        / track_count.max(1) as f32;
    let core: Vec<_> = genre_distribution
        .iter()
        .filter(|(genre, _)| !is_unknown_genre(genre))
        .take(3)
        .map(|v| v.0.clone())
        .collect();
    let adjacent = adjacent_genres(&core);
    let unexplored = exploration_genres(&core, &adjacent);

    TasteReport {
        playlist_name: playlist.name.clone(),
        source_label: playlist.source.clone(),
        is_demo: playlist.is_demo,
        track_count,
        genre_distribution,
        artist_distribution,
        era_distribution,
        album_distribution,
        duplicate_track_count,
        collaboration_track_count,
        genre_matched_count,
        genre_coverage,
        energy_matched_count,
        energy_coverage,
        metrics: vec![
            metric(
                "Genre 多样性",
                genre_diversity,
                format!("{:.0}%", genre_diversity * 100.0),
                "不同 Genre 相对曲目规模的覆盖程度",
            ),
            metric(
                "歌手集中度",
                artist_concentration,
                format!("{:.0}%", artist_concentration * 100.0),
                "最高频歌手占全部曲目的比例，越低越分散",
            ),
            metric(
                "平均能量",
                average_energy,
                if energies.is_empty() {
                    "数据不足".into()
                } else {
                    format!("{:.0}/100", average_energy * 100.0)
                },
                "来自曲目元数据的能量均值，不代表心理状态",
            ),
            metric(
                "Energy 覆盖率",
                energy_coverage,
                format!("{energy_matched_count}/{track_count}"),
                "仅统计输入文件或元数据 Provider 真实提供 Energy 的歌曲",
            ),
            metric(
                "元数据置信度",
                confidence,
                format!("{:.0}%", confidence * 100.0),
                "分析字段完整度和来源可信度的综合估计",
            ),
            metric(
                "Genre 覆盖率",
                genre_coverage,
                format!("{genre_matched_count}/{track_count}"),
                "仅统计真实获得 Genre 的歌曲",
            ),
        ],
        core_preferences: core.clone(),
        adjacent_preferences: adjacent.clone(),
        unexplored_preferences: unexplored,
        summary: if playlist.is_demo {
            "这份示例歌单以 Korean R&B 与 K-pop 为稳定核心，同时在 City Pop 的柔和律动中保留了向 Neo Soul 过渡的空间。整体偏低至中等能量，适合沿音色与律动逐步拓展。".into()
        } else if !core.is_empty() {
            format!(
                "已在本机完成 {} 首歌曲的分析。当前最明显的声音核心是 {}；报告结合公开音乐目录与本地知识库补全元数据，未登录任何音乐平台账号。",
                track_count,
                core.join("、")
            )
        } else {
            "已完成歌手与曲目统计，但部分歌曲没有匹配到可用元数据；结果会明确标注低置信度，不做过度推断。".into()
        },
        confidence,
        limitations: if playlist.is_demo {
            vec!["Genre、情绪和能量为离线演示元数据，不是实时平台分析。".into()]
        } else {
            let unknown = playlist
                .tracks
                .iter()
                .filter(|track| track.genres.iter().any(|genre| genre.contains("待补全")))
                .count();
            let mut notes = vec![
                "分析在本机运行；联网时只查询公开音乐目录，不需要平台账号、密码或 Cookie。".into(),
            ];
            if unknown > 0 {
                notes.push(format!("有 {unknown} 首歌曲未匹配到完整元数据，其 Genre、年代或能量指标不会被强行猜测。"));
            }
            notes
        },
    }
}

pub fn adjacent_genres(core: &[String]) -> Vec<String> {
    crate::genre::adjacent_genres(core)
}

fn metric(label: &str, value: f32, display: String, explanation: &str) -> TasteMetric {
    TasteMetric {
        label: label.into(),
        value,
        display,
        explanation: explanation.into(),
    }
}

pub fn build_personal_demo(playlist: Playlist) -> PersonalDemo {
    let report = analyze_playlist(&playlist);
    let all = catalog();
    let recommendation_specs = [
        (
            "Tadow",
            "Masego & FKJ",
            "舒适区",
            0.94,
            0.42,
            "延续 DEAN 与 Colde 作品中的松弛 R&B 律动",
            "Korean R&B 的人声留白与爵士和弦",
            "引入更鲜明的萨克斯即兴",
        ),
        (
            "Square (2017)",
            "Yerin Baek",
            "舒适区",
            0.92,
            0.38,
            "保留熟悉的韩国独立 R&B 气质",
            "与 BIBI 的克制叙事感相连",
            "拓展英文创作与独立流行侧面",
        ),
        (
            "D (Half Moon)",
            "DEAN",
            "舒适区",
            0.91,
            0.31,
            "以熟悉歌手作为低风险入口",
            "直接连接歌单中的 Korean R&B 核心",
            "补全更轻盈的都市夜行情绪",
        ),
        (
            "Get You",
            "Daniel Caesar",
            "拓展区",
            0.86,
            0.65,
            "从细腻 R&B 人声自然过渡到 Neo Soul",
            "与 Crush 的暖色和声结构相近",
            "首次引入北美 Neo Soul",
        ),
        (
            "Japanese Denim",
            "Daniel Caesar",
            "拓展区",
            0.84,
            0.68,
            "慢速律动与木质音色不会造成突兀跳跃",
            "连接 City Pop 的柔和贝斯与 R&B 演唱",
            "拓展原声质感",
        ),
        (
            "Make It Better",
            "Anderson .Paak",
            "拓展区",
            0.81,
            0.72,
            "保持律动感，同时提高 Soul 与 Funk 成分",
            "与 Zion.T 的复古律动相关",
            "进入更鲜活的 Neo Soul 乐队编制",
        ),
        (
            "Cranes in the Sky",
            "Solange",
            "拓展区",
            0.79,
            0.76,
            "以极简编曲连接低能量偏好",
            "与 Heize 的空间感和克制人声相连",
            "拓展艺术 R&B 与 Neo Soul",
        ),
        (
            "Sober",
            "Childish Gambino",
            "惊喜区",
            0.70,
            0.86,
            "用 Soul 唱腔跨向另类电子制作",
            "仍保留用户熟悉的流畅旋律",
            "提高合成器与实验性占比",
        ),
        (
            "Them Changes",
            "Thundercat",
            "惊喜区",
            0.68,
            0.91,
            "从 R&B 贝斯线进入 Jazz-Funk",
            "与 City Pop 的复杂和弦共享语言",
            "扩展到高速贝斯与迷幻 Funk",
        ),
        (
            "Pink + White",
            "Frank Ocean",
            "惊喜区",
            0.76,
            0.83,
            "通过柔和音色进入更开放的 Alternative R&B",
            "延续低中能量和细腻氛围",
            "增加非线性叙事与实验结构",
        ),
    ];
    let recommendations: Vec<Recommendation> = recommendation_specs
        .iter()
        .filter_map(
            |(title, artist, zone, score, novelty, reason, connection, expansion)| {
                all.iter()
                    .find(|t| t.title == *title && t.artists.iter().any(|a| a == artist))
                    .cloned()
                    .map(|track| Recommendation {
                        track,
                        zone: (*zone).into(),
                        reason: (*reason).into(),
                        connection: (*connection).into(),
                        expansion: (*expansion).into(),
                        match_score: *score,
                        novelty_score: *novelty,
                        candidate_source: "DEMO 内置候选".into(),
                        match_confidence: *score,
                        source_endpoint: "demo.catalog".into(),
                        seed_track: None,
                        seed_artist: None,
                        lastfm_similarity: None,
                        tags: Vec::new(),
                        relaxation_level: 0,
                        already_in_source_playlist: false,
                    })
            },
        )
        .collect();

    let route_genres = [
        (
            "Korean R&B",
            "从熟悉的人声质感与低中速律动出发",
            ["Square (2017)", "D (Half Moon)"],
        ),
        (
            "Alternative R&B",
            "保留 R&B 骨架，增加更自由的制作与叙事",
            ["Pink + White", "Sober"],
        ),
        (
            "Neo Soul",
            "沿复杂和声、真实乐器和松弛律动继续深入",
            ["Get You", "Cranes in the Sky"],
        ),
    ];
    let route = route_genres
        .into_iter()
        .map(|(genre, explanation, titles)| RouteStep {
            genre: genre.into(),
            explanation: explanation.into(),
            tracks: titles
                .iter()
                .filter_map(|title| all.iter().find(|t| t.title == *title).cloned())
                .collect(),
        })
        .collect();

    let comfort_pool = recommendations
        .iter()
        .filter(|item| item.zone == "舒适区")
        .cloned()
        .collect();
    let expansion_pool = recommendations
        .iter()
        .filter(|item| item.zone == "拓展区")
        .cloned()
        .collect();
    let surprise_pool = recommendations
        .iter()
        .filter(|item| item.zone == "惊喜区")
        .cloned()
        .collect();
    PersonalDemo {
        analysis_id: format!("demo:{}", playlist.id),
        playlist,
        report,
        recommendations,
        route,
        recommendation_summary: RecommendationSummary {
            source_label: "DEMO 内置候选".into(),
            status: "demo".into(),
            message: "这是明确标注的 Demo 推荐，不用于真实文件或真实文本分析。".into(),
            candidate_count: 10,
            seeds: Vec::new(),
            query_stats: Default::default(),
            zones: vec![
                RecommendationZoneSummary {
                    zone: "舒适区".into(),
                    count: 3,
                    message: "Demo 候选".into(),
                },
                RecommendationZoneSummary {
                    zone: "拓展区".into(),
                    count: 4,
                    message: "Demo 候选".into(),
                },
                RecommendationZoneSummary {
                    zone: "惊喜区".into(),
                    count: 3,
                    message: "Demo 候选".into(),
                },
            ],
            comfort_pool,
            expansion_pool,
            surprise_pool,
        },
        import_summary: None,
        unmatched_tracks: Vec::new(),
        metadata_resolutions: Vec::new(),
    }
}

pub fn compare_playlists(a: &Playlist, b: &Playlist) -> ComparisonReport {
    let mut report = comparison_metrics(a, b);
    if a.is_demo && b.is_demo {
        report.bridge_playlist = demo_bridge_playlist();
    }
    report
}

pub fn compare_analyses(a: &PersonalDemo, b: &PersonalDemo) -> ComparisonReport {
    let mut report = comparison_metrics(&a.playlist, &b.playlist);
    let mut candidates: HashMap<String, MutualCandidate> = HashMap::new();
    for (recommendations, from_a) in [(&a.recommendations, true), (&b.recommendations, false)] {
        for recommendation in recommendations {
            if a.playlist
                .tracks
                .iter()
                .chain(&b.playlist.tracks)
                .any(|source| same_recording(source, &recommendation.track))
            {
                continue;
            }
            let key = normalized_track_key(&recommendation.track);
            let affinity_a = if from_a {
                recommendation.match_score
            } else {
                profile_affinity(&recommendation.track, &a.report, &a.playlist)
            };
            let affinity_b = if from_a {
                profile_affinity(&recommendation.track, &b.report, &b.playlist)
            } else {
                recommendation.match_score
            };
            candidates
                .entry(key)
                .and_modify(|candidate| {
                    candidate.score_a = candidate.score_a.max(affinity_a);
                    candidate.score_b = candidate.score_b.max(affinity_b);
                    if !candidate.source.contains(&recommendation.candidate_source) {
                        candidate.source.push_str(" + ");
                        candidate.source.push_str(&recommendation.candidate_source);
                    }
                })
                .or_insert_with(|| MutualCandidate {
                    track: recommendation.track.clone(),
                    source: recommendation.candidate_source.clone(),
                    score_a: affinity_a,
                    score_b: affinity_b,
                });
        }
    }
    let mut ranked: Vec<_> = candidates
        .into_values()
        .filter_map(|candidate| mutual_bridge_track(candidate, &a.report, &b.report))
        .collect();
    ranked.sort_by(|left, right| right.bridge_score.total_cmp(&left.bridge_score));
    let mut zone_counts: HashMap<String, usize> = HashMap::new();
    let mut artist_counts: HashMap<(String, String), usize> = HashMap::new();
    report.bridge_playlist = ranked
        .into_iter()
        .filter(|item| {
            let zone_count = zone_counts.entry(item.phase.clone()).or_default();
            let artist = item
                .track
                .artists
                .first()
                .map(|value| normalize_text(value))
                .unwrap_or_default();
            let artist_count = artist_counts
                .entry((item.phase.clone(), artist))
                .or_default();
            if *zone_count >= 4 || *artist_count >= 2 {
                return false;
            }
            *zone_count += 1;
            *artist_count += 1;
            true
        })
        .collect();
    report.summary = if report.bridge_playlist.is_empty() {
        format!(
            "已完成两份真实歌单的确定性比较；当前没有同时满足双方关联和双源排除条件的真实推荐候选。{}",
            if a.recommendation_summary.status == "not_configured"
                || b.recommendation_summary.status == "not_configured"
            {
                " Last.fm 未配置，因此不会用 Demo 补齐共同推荐。"
            } else {
                " 候选不足时保持为空。"
            }
        )
    } else {
        format!(
            "两份歌单共发现 {} 首共同歌曲、{} 位共同艺人和 {} 个共同 Genre；共同推荐严格排除了双方源歌单。",
            report.shared_track_count,
            report.shared_artists.len(),
            report.shared_genres.len()
        )
    };
    report
}

#[derive(Clone)]
struct MutualCandidate {
    track: Track,
    source: String,
    score_a: f32,
    score_b: f32,
}

fn comparison_metrics(a: &Playlist, b: &Playlist) -> ComparisonReport {
    let a_report = analyze_playlist(a);
    let b_report = analyze_playlist(b);
    let a_track_keys: HashSet<_> = a.tracks.iter().map(normalized_track_key).collect();
    let b_track_keys: HashSet<_> = b.tracks.iter().map(normalized_track_key).collect();
    let shared_track_count = a_track_keys.intersection(&b_track_keys).count();
    let track_overlap = jaccard(&a_track_keys, &b_track_keys);
    let a_artist_keys: HashSet<_> = a.tracks.iter().flat_map(artist_identity_keys).collect();
    let b_artist_keys: HashSet<_> = b.tracks.iter().flat_map(artist_identity_keys).collect();
    let artist_overlap = jaccard(&a_artist_keys, &b_artist_keys);
    let mut shared_artists: Vec<_> = a_artist_keys
        .intersection(&b_artist_keys)
        .filter_map(|value| value.strip_prefix("name:"))
        .map(str::to_string)
        .collect();
    shared_artists.sort();
    let genre_set = |playlist: &Playlist| {
        playlist
            .tracks
            .iter()
            .flat_map(|track| track.genres.iter())
            .filter(|genre| !is_unknown_genre(genre))
            .map(|genre| canonicalize_genre(genre))
            .collect::<HashSet<_>>()
    };
    let a_genres = genre_set(a);
    let b_genres = genre_set(b);
    let genre_overlap = jaccard(&a_genres, &b_genres);
    let mut shared_genres: Vec<_> = a_genres.intersection(&b_genres).cloned().collect();
    shared_genres.sort();
    let tag_set = |playlist: &Playlist| {
        playlist
            .tracks
            .iter()
            .flat_map(|track| track.mood_tags.iter())
            .map(|tag| normalize_text(tag))
            .filter(|tag| !tag.is_empty())
            .collect::<HashSet<_>>()
    };
    let tag_overlap = jaccard(&tag_set(a), &tag_set(b));
    let diversity_a = a_report
        .metrics
        .first()
        .map(|metric| metric.value)
        .unwrap_or(0.0);
    let diversity_b = b_report
        .metrics
        .first()
        .map(|metric| metric.value)
        .unwrap_or(0.0);
    let diversity_complementarity = (1.0 - (diversity_a - diversity_b).abs()).clamp(0.0, 1.0);
    let similarity = (track_overlap * 0.28
        + artist_overlap * 0.25
        + genre_overlap * 0.27
        + tag_overlap * 0.1
        + diversity_complementarity * 0.1)
        .clamp(0.0, 1.0);
    let signatures = |report: &TasteReport| {
        let mut values = report.core_preferences.clone();
        if let Some(energy) = report
            .metrics
            .iter()
            .find(|metric| metric.label == "平均能量")
        {
            values.push(format!("Energy {}", energy.display));
        }
        values.truncate(4);
        values
    };
    ComparisonReport {
        user_a: a.name.clone(),
        user_b: b.name.clone(),
        metrics: vec![
            metric(
                "Overall Compatibility",
                similarity,
                format!("{:.0}%", similarity * 100.0),
                "曲目、艺人、Genre、Tag 与多样性互补度的确定性加权结果",
            ),
            metric(
                "Track Overlap",
                track_overlap,
                format!("{:.0}%", track_overlap * 100.0),
                "规范化曲目身份的 Jaccard 重合度",
            ),
            metric(
                "Artist Overlap",
                artist_overlap,
                format!("{:.0}%", artist_overlap * 100.0),
                "含集中别名解析的艺人身份重合度",
            ),
            metric(
                "Genre Overlap",
                genre_overlap,
                format!("{:.0}%", genre_overlap * 100.0),
                "规范化 Genre 集合的 Jaccard 重合度",
            ),
            metric(
                "Tag Overlap",
                tag_overlap,
                format!("{:.0}%", tag_overlap * 100.0),
                "真实元数据 Tag 集合的重合度",
            ),
            metric(
                "Diversity Complementarity",
                diversity_complementarity,
                format!("{:.0}%", diversity_complementarity * 100.0),
                "双方多样性差距越小，基础兼容度越高",
            ),
        ],
        track_count_a: a.tracks.len(),
        track_count_b: b.tracks.len(),
        shared_track_count,
        shared_artists,
        shared_genres,
        user_a_signatures: signatures(&a_report),
        user_b_signatures: signatures(&b_report),
        summary: format!(
            "已比较 {} 与 {} 的真实曲目、艺人、Genre、Tag 和多样性。",
            a.name, b.name
        ),
        bridge_playlist: Vec::new(),
        is_demo: a.is_demo || b.is_demo,
        data_source: if a.is_demo || b.is_demo {
            "DEMO".into()
        } else {
            "REAL_TEMPORARY_COMPARISON".into()
        },
        saved_locally: false,
    }
}

fn mutual_bridge_track(
    candidate: MutualCandidate,
    report_a: &TasteReport,
    report_b: &TasteReport,
) -> Option<BridgeTrack> {
    let minimum = candidate.score_a.min(candidate.score_b);
    let maximum = candidate.score_a.max(candidate.score_b);
    let phase = if minimum >= 0.48 {
        "Safe for Both"
    } else if minimum >= 0.16 && maximum >= 0.48 {
        "Bridge"
    } else if minimum >= 0.14 && maximum >= 0.25 {
        "Adventure Together"
    } else {
        return None;
    };
    let candidate_genres: HashSet<_> = candidate
        .track
        .genres
        .iter()
        .map(|genre| genre_key(genre))
        .collect();
    let mut shared_basis: Vec<_> = report_a
        .core_preferences
        .iter()
        .chain(&report_b.core_preferences)
        .filter(|genre| candidate_genres.contains(&genre_key(genre)))
        .cloned()
        .collect();
    shared_basis.sort();
    shared_basis.dedup();
    let score = ((candidate.score_a + candidate.score_b) / 2.0).clamp(0.0, 0.99);
    Some(BridgeTrack {
        track: candidate.track,
        reason: format!(
            "A 关联 {:.0}% · B 关联 {:.0}%",
            candidate.score_a * 100.0,
            candidate.score_b * 100.0
        ),
        reason_for_a: profile_reason(candidate.score_a, report_a),
        reason_for_b: profile_reason(candidate.score_b, report_b),
        shared_basis,
        candidate_source: candidate.source,
        phase: phase.into(),
        bridge_score: score,
        already_in_a: false,
        already_in_b: false,
    })
}

fn profile_affinity(track: &Track, report: &TasteReport, playlist: &Playlist) -> f32 {
    let core: HashSet<_> = report
        .core_preferences
        .iter()
        .map(|genre| genre_key(genre))
        .collect();
    let genre_match = track
        .genres
        .iter()
        .any(|genre| core.contains(&genre_key(genre)));
    let source_artists: HashSet<_> = playlist
        .tracks
        .iter()
        .flat_map(artist_identity_keys)
        .collect();
    let artist_match = artist_identity_keys(track)
        .iter()
        .any(|artist| source_artists.contains(artist));
    (if genre_match { 0.55 } else { 0.14 }) + if artist_match { 0.28 } else { 0.0 }
}

fn profile_reason(score: f32, report: &TasteReport) -> String {
    format!(
        "与 {} 的核心偏好关联 {:.0}%",
        report.core_preferences.join(" / "),
        score * 100.0
    )
}

fn jaccard<T: Eq + std::hash::Hash>(left: &HashSet<T>, right: &HashSet<T>) -> f32 {
    if left.is_empty() && right.is_empty() {
        return 0.0;
    }
    left.intersection(right).count() as f32 / left.union(right).count().max(1) as f32
}

fn demo_bridge_playlist() -> Vec<BridgeTrack> {
    let specifications = [
        (
            "Dreams",
            "Safe for Both",
            "柔和旋律与中等能量为双方建立安全起点",
            0.92,
        ),
        ("Tadow", "Safe for Both", "R&B 律动中加入器乐互动", 0.89),
        ("Sober", "Bridge", "另类电子制作连接 R&B 与 Indie", 0.87),
        (
            "Electric Feel",
            "Bridge",
            "合成器与节拍覆盖双方示例偏好",
            0.90,
        ),
        (
            "The Less I Know the Better",
            "Bridge",
            "Soul/Funk 平滑进入 Indie Rock",
            0.91,
        ),
        (
            "Pink + White",
            "Adventure Together",
            "示例中的 Alternative R&B 共同探索",
            0.86,
        ),
        (
            "505",
            "Adventure Together",
            "示例中的 Alternative Rock 共同探索",
            0.84,
        ),
        (
            "Somebody Else",
            "Adventure Together",
            "示例中的合成器与 Indie 叙事",
            0.93,
        ),
    ];
    let catalog = catalog();
    specifications
        .iter()
        .filter_map(|(title, phase, reason, score)| {
            catalog
                .iter()
                .find(|track| track.title == *title)
                .cloned()
                .map(|track| BridgeTrack {
                    track,
                    reason: (*reason).into(),
                    reason_for_a: "Demo A 解释".into(),
                    reason_for_b: "Demo B 解释".into(),
                    shared_basis: vec!["DEMO".into()],
                    candidate_source: "DEMO 内置候选".into(),
                    phase: (*phase).into(),
                    bridge_score: *score,
                    already_in_a: false,
                    already_in_b: false,
                })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::demo::demo_playlists;

    #[test]
    fn parses_manual_lines_and_rejects_invalid_input() {
        let playlist = parse_manual_playlist("test", "BIBI - Kazino\nDEAN — instagram").unwrap();
        assert_eq!(playlist.tracks.len(), 2);
        assert!(parse_manual_playlist("bad", "only a title").is_err());
    }

    #[test]
    fn report_has_calculated_metrics() {
        let report = analyze_playlist(&demo_playlists()[0]);
        assert_eq!(report.track_count, 8);
        assert!(
            report
                .metrics
                .iter()
                .all(|m| (0.0..=1.0).contains(&m.value))
        );
    }

    #[test]
    fn recommendations_exclude_source_tracks_and_limit_artist_repetition() {
        let demo = build_personal_demo(demo_playlists()[0].clone());
        let source_ids: HashSet<_> = demo.playlist.tracks.iter().map(|t| &t.id).collect();
        assert!(
            demo.recommendations
                .iter()
                .all(|r| !source_ids.contains(&r.track.id))
        );
        let mut counts = HashMap::new();
        for recommendation in &demo.recommendations {
            *counts.entry(&recommendation.track.artists[0]).or_insert(0) += 1;
        }
        assert!(counts.values().all(|count| *count <= 2));
    }

    #[test]
    fn bridge_has_smooth_phases_and_no_adjacent_artist_duplicates() {
        let lists = demo_playlists();
        let report = compare_playlists(&lists[0], &lists[1]);
        assert!(report.bridge_playlist.len() >= 6);
        assert!(
            report
                .bridge_playlist
                .windows(2)
                .all(|pair| { pair[0].track.artists.first() != pair[1].track.artists.first() })
        );
        assert!(report.bridge_playlist.iter().all(|t| !t.reason.is_empty()));
    }

    #[test]
    fn compares_unified_tracks_from_different_permitted_sources() {
        let mut lists = demo_playlists();
        lists[0].source = "manual".into();
        lists[1].source = "file".into();
        for track in &mut lists[0].tracks {
            track.platform = "manual".into();
        }
        for track in &mut lists[1].tracks {
            track.platform = "file".into();
        }
        let report = compare_playlists(&lists[0], &lists[1]);
        assert_eq!(report.user_a, lists[0].name);
        assert_eq!(report.user_b, lists[1].name);
        assert_eq!(report.bridge_playlist.len(), 8);
    }

    #[test]
    fn identical_playlists_have_full_track_and_artist_overlap() {
        let playlist = demo_playlists()[0].clone();
        let report = compare_playlists(&playlist, &playlist);
        let track_overlap = report
            .metrics
            .iter()
            .find(|metric| metric.label == "Track Overlap")
            .unwrap();
        let artist_overlap = report
            .metrics
            .iter()
            .find(|metric| metric.label == "Artist Overlap")
            .unwrap();
        assert_eq!(track_overlap.value, 1.0);
        assert_eq!(artist_overlap.value, 1.0);
        assert_eq!(report.shared_track_count, playlist.tracks.len());
    }

    #[test]
    fn completely_different_playlists_explain_low_direct_overlap() {
        let first = parse_manual_playlist("中文", "周杰伦 - 晴天\n林俊杰 - 背对背拥抱").unwrap();
        let second =
            parse_manual_playlist("English", "Miles Davis - So What\nJohn Coltrane - Naima")
                .unwrap();
        let report = compare_playlists(&first, &second);
        assert_eq!(report.shared_track_count, 0);
        assert!(report.shared_artists.is_empty());
        assert!(!report.is_demo);
    }

    #[test]
    fn partial_overlap_is_counted_without_inventing_shared_tracks() {
        let first = parse_manual_playlist(
            "A",
            "BIBI - Kazino\nDEAN - instagram\nThe 1975 - Somebody Else",
        )
        .unwrap();
        let second = parse_manual_playlist(
            "B",
            "DEAN - instagram\nNewJeans - Super Shy\nDua Lipa - Levitating",
        )
        .unwrap();
        let report = compare_playlists(&first, &second);
        assert_eq!(report.shared_track_count, 1);
    }

    #[test]
    fn mutual_recommendations_exclude_both_source_playlists() {
        let lists = demo_playlists();
        let mut analysis_a = build_personal_demo(lists[0].clone());
        let mut analysis_b = build_personal_demo(lists[1].clone());
        analysis_a.playlist.is_demo = false;
        analysis_b.playlist.is_demo = false;
        let report = compare_analyses(&analysis_a, &analysis_b);
        assert!(report.bridge_playlist.iter().all(|item| {
            !analysis_a
                .playlist
                .tracks
                .iter()
                .chain(&analysis_b.playlist.tracks)
                .any(|source| same_recording(source, &item.track))
                && !item.already_in_a
                && !item.already_in_b
        }));
    }
}
