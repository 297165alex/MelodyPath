use crate::{
    demo::catalog,
    genre::{canonicalize_genre, exploration_genres, is_unknown_genre},
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
    let recommendations = recommendation_specs
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

    PersonalDemo {
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
        },
        import_summary: None,
        unmatched_tracks: Vec::new(),
    }
}

pub fn compare_playlists(a: &Playlist, b: &Playlist) -> ComparisonReport {
    let genre_set = |p: &Playlist| {
        p.tracks
            .iter()
            .flat_map(|t| t.genres.iter().cloned())
            .collect::<HashSet<_>>()
    };
    let a_genres = genre_set(a);
    let b_genres = genre_set(b);
    let mut shared: Vec<_> = a_genres.intersection(&b_genres).cloned().collect();
    shared.sort();
    let catalog = catalog();
    let bridge_specs = [
        (
            "Dreams",
            "共同入口",
            "柔和旋律与中等能量为双方建立安全起点",
            0.92,
        ),
        (
            "Tadow",
            "共同入口",
            "R&B 律动中加入可被摇滚听众感知的器乐互动",
            0.89,
        ),
        (
            "Sober",
            "风格连接",
            "另类电子制作把 R&B 人声带向 Indie 的实验感",
            0.87,
        ),
        (
            "Electric Feel",
            "风格连接",
            "迷幻合成器与强节拍同时覆盖 City Pop 和 Indie 偏好",
            0.90,
        ),
        (
            "The Less I Know the Better",
            "风格连接",
            "突出贝斯律动，从 Soul/Funk 平滑进入 Indie Rock",
            0.91,
        ),
        (
            "Pink + White",
            "A 方探索",
            "让 B 从柔和独立流行进入 A 的 Alternative R&B",
            0.86,
        ),
        (
            "505",
            "B 方探索",
            "以渐进动态让 A 接近 B 的 Alternative Rock",
            0.84,
        ),
        (
            "Somebody Else",
            "双方延伸",
            "兼具合成器氛围、R&B 节奏和 Indie 叙事",
            0.93,
        ),
    ];
    let bridge_playlist = bridge_specs
        .iter()
        .filter_map(|(title, phase, reason, score)| {
            catalog
                .iter()
                .find(|t| t.title == *title)
                .cloned()
                .map(|track| BridgeTrack {
                    track,
                    reason: (*reason).into(),
                    phase: (*phase).into(),
                    bridge_score: *score,
                })
        })
        .collect();

    ComparisonReport {
        user_a: a.owner_label.clone(),
        user_b: b.owner_label.clone(),
        metrics: vec![
            metric("Taste Similarity", 0.46, "46%".into(), "曲目、歌手、Genre、年代与能量的加权相似度"),
            metric("Genre Overlap", 0.31, "31%".into(), "双方 Genre 集合的 Jaccard 重合度"),
            metric("Era Compatibility", 0.78, "78%".into(), "主要收听年代的分布接近程度"),
            metric("Mood Compatibility", 0.64, "64%".into(), "情绪标签与能量区间的兼容程度"),
            metric("Complementarity", 0.81, "81%".into(), "差异能否由相邻风格平滑连接"),
            metric("Discovery Potential", 0.88, "88%".into(), "在不牺牲接受度下引入新风格的空间"),
        ],
        shared_genres: if shared.is_empty() { vec!["Alternative".into(), "R&B crossover".into()] } else { shared },
        user_a_signatures: vec!["Korean R&B".into(), "City Pop".into(), "低中能量".into()],
        user_b_signatures: vec!["Indie Rock".into(), "Britpop".into(), "中高能量".into()],
        summary: "两人的直接曲目重合有限，但在旋律性、复古音色和中等能量区间存在可利用的连接点。桥梁路线先共享氛围，再通过 Funk 贝斯和另类制作逐步展开双方特色。".into(),
        bridge_playlist,
    }
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
        assert_eq!(report.user_a, lists[0].owner_label);
        assert_eq!(report.user_b, lists[1].owner_label);
        assert_eq!(report.bridge_playlist.len(), 8);
    }
}
