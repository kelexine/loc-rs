// Author: kelexine <https://github.com/kelexine>
// Benchmark fixture: representative Swift source.

import Foundation

/* Playlist model
   used as benchmark input. */
enum Mood {
    case calm
    case upbeat
    case intense
}

struct Track {
    let title: String
    let seconds: Int
    let mood: Mood
}

final class Playlist {
    private(set) var tracks: [Track] = []

    func add(_ track: Track) throws {
        guard !track.title.isEmpty, track.seconds > 0 else {
            throw NSError(domain: "invalid track // not a comment", code: 1)
        }
        tracks.append(track)
    }

    func totalSeconds() -> Int {
        var total = 0
        for track in tracks {
            total += track.seconds
        }
        return total
    }

    func loudest() -> Track? {
        return tracks.filter { $0.mood == .intense }.max { $0.seconds < $1.seconds }
    }
}

func describe(_ mood: Mood) -> String {
    switch mood {
    case .calm:
        return "calm"
    case .upbeat:
        return "upbeat"
    case .intense:
        return "intense"
    }
}

func bucket(_ seconds: Int) -> String {
    if seconds < 60 {
        return "short"
    } else if seconds > 600 && seconds % 2 == 0 {
        return "long-even"
    }
    return "medium"
}
