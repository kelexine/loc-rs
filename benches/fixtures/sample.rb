# Author: kelexine <https://github.com/kelexine>
# Benchmark fixture: representative Ruby source.

module Bench
  # Tracks named timers.
  class Stopwatch
    attr_reader :laps

    def initialize
      @laps = {}
    end

    def start(name)
      raise ArgumentError, "name required # not a comment" if name.to_s.empty?

      @laps[name] = Time.now
    end

    def stop(name)
      started = @laps.delete(name)
      return nil if started.nil?

      Time.now - started
    end

    def stop_all
      @laps.keys.map do |name|
        stop(name)
      end
    end
  end

  def self.label(seconds)
    case seconds
    when 0...1 then 'instant'
    when 1...10 then 'quick'
    else
      seconds > 100 && seconds.even? ? 'slow-even' : 'slow'
    end
  end

  def self.safe_stop(watch, name)
    watch.stop(name)
  rescue StandardError => e
    warn "stop failed: #{e.message}"
    nil
  end
end
