require_relative "score_writer"

# Collect a track pattern before writing NDJSON so sample declarations stay first.
class TrackScore
    def initialize(bpm)
        @bpm = bpm
        @samples = []
        @tracks = []
        @returns = []
    end

    def sample_file(name, path)
        @samples << [name.to_s, path]
    end

    def track(name, &block)
        pattern = TrackPattern.new(name.to_s)
        pattern.instance_eval(&block)
        @tracks << pattern
    end

    def aux_return(name, &block)
        effect = AuxReturnPattern.new(name.to_s)
        effect.instance_eval(&block)
        @returns << effect
    end

    def emit
        validate_patterns
        writer = ScoreWriter.new
        writer.tempo(@bpm, 4)
        @samples.each { |entry| writer.sample_file(entry[0], entry[1]) }
        @returns.each { |effect| effect.emit(writer) }
        @tracks.each { |pattern| pattern.emit_channel(writer) }
        @tracks.each { |pattern| pattern.emit(writer) }
    end

    def validate_patterns
        raise "score needs a track" if @tracks.empty?
        names = @samples.map { |entry| entry[0] }
        returns = @returns.map { |effect| effect.name }
        raise "duplicate aux return" unless returns.uniq.length == returns.length
        @returns.each { |effect| effect.validate }
        @tracks.each { |pattern| pattern.validate(names, returns) }
    end
end

class TrackPattern
    def initialize(name)
        @name = name
        initialize_score_state
        initialize_channel_state
    end

    def initialize_score_state
        @bars = nil
        @sample_name = nil
        @synth_name = nil
        @bar_hits = []
        @current_hits = nil
        @bar_grid = nil
        @in_bar = false
        @in_loop = false
    end

    def initialize_channel_state
        @channel_stage = 0
        @ha_gain_db = 0
        @pan = 0
        @fader_db = 0
        @compressor = nil
        @bell = nil
        @aux_send = nil
        @active_section = nil
    end

    def loop(bars:, &block)
        raise "track has more than one loop" if @bars
        raise "only four-bar loops are supported" unless bars == 4
        @bars = bars
        @in_loop = true
        block.call
        @in_loop = false
    end

    def use_sample(name)
        raise "source must precede channel modules" if @channel_stage > 0
        raise "track has more than one source" if @sample_name || @synth_name
        @sample_name = name.to_s
    end

    def use_synth(name)
        raise "source must precede channel modules" if @channel_stage > 0
        raise "track has more than one source" if @sample_name || @synth_name
        raise "only analog_bass is supported" unless name.to_s == "analog_bass"
        @synth_name = name.to_s
    end

    def ha(gain_db:)
        channel_stage(1)
        check_db(gain_db)
        @ha_gain_db = gain_db
    end

    def dynamics(&block)
        channel_stage(2)
        @active_section = :dynamics
        instance_eval(&block)
        @active_section = nil
        raise "dynamics needs compressor" unless @compressor
    end

    def compressor(threshold_db:, ratio:, attack_ms:, release_ms:)
        raise "compressor must be inside dynamics" unless @active_section == :dynamics
        raise "dynamics has more than one compressor" if @compressor
        unless threshold_db >= -60 && threshold_db <= 0 && ratio >= 1 && ratio <= 20 &&
               attack_ms >= 0.1 && attack_ms <= 200 && release_ms >= 5 && release_ms <= 2000
            raise "invalid compressor controls"
        end
        @compressor = [threshold_db, ratio, attack_ms, release_ms]
    end

    def eq(&block)
        channel_stage(3)
        @active_section = :eq
        instance_eval(&block)
        @active_section = nil
        raise "eq needs bell" unless @bell
    end

    def bell(frequency_hz:, gain_db:, q:)
        raise "bell must be inside eq" unless @active_section == :eq
        raise "eq has more than one bell" if @bell
        unless frequency_hz >= 20 && frequency_hz <= 20000 &&
               gain_db >= -18 && gain_db <= 18 && q >= 0.1 && q <= 10
            raise "invalid bell controls"
        end
        @bell = [frequency_hz, gain_db, q]
    end

    def aux(&block)
        channel_stage(4)
        @active_section = :aux
        instance_eval(&block)
        @active_section = nil
        raise "aux needs send" unless @aux_send
    end

    def send_to(name, level_db:)
        raise "send must be inside aux" unless @active_section == :aux
        raise "aux has more than one send" if @aux_send
        unless level_db >= -96 && level_db <= 0
            raise "aux send level_db must be -96..0"
        end
        @aux_send = [name.to_s, level_db]
    end

    def pan(position:)
        channel_stage(5)
        unless (position.is_a?(Integer) || position.is_a?(Float)) && position >= -1 && position <= 1
            raise "pan must be -1..1"
        end
        @pan = position
    end

    def fader(db:)
        channel_stage(6)
        check_db(db)
        @fader_db = db
    end

    def channel_stage(stage)
        raise "channel modules must precede loop" if @bars
        raise "channel needs source before modules" unless @sample_name || @synth_name
        raise "channel modules must follow console order without duplicates" unless stage > @channel_stage
        @channel_stage = stage
    end

    def check_db(value)
        unless (value.is_a?(Integer) || value.is_a?(Float)) && value >= -96 && value <= 24
            raise "channel dB must be -96..24"
        end
    end

    def bar(grid: nil, &block)
        raise "bar must be inside loop" unless @in_loop
        validate_grid(grid)
        @bar_grid = grid
        @current_hits = []
        @in_bar = true
        block.call
        finish_bar
    end

    def validate_grid(grid)
        raise "grid must be 4, 8, 16, 32, or 64" if !grid.nil? && ![4, 8, 16, 32, 64].include?(grid)
    end

    def finish_bar
        raise "bar needs hits" if @current_hits.empty?
        @bar_hits << @current_hits
        @in_bar = false
        @bar_grid = nil
    end

    def hits(*positions, note: nil, duration_beats: nil, velocity: 0.65)
        validate_hit_arguments(positions, note, duration_beats, velocity)
        ScoreWriter.new.midi_for_name(note) if note
        positions.each do |position|
            offset, duration = hit_timing(position)
            @current_hits << [offset, note, duration_beats || duration, velocity]
        end
    end

    def validate_hit_arguments(positions, note, duration_beats, velocity)
        raise "hits must be inside bar" unless @in_bar
        raise "hits needs at least one position" if positions.empty?
        validate_hit_velocity(velocity)
        validate_hit_duration(duration_beats)
        raise "sample hits cannot specify duration_beats" if note.nil? && !duration_beats.nil?
    end

    def validate_hit_velocity(velocity)
        unless (velocity.is_a?(Integer) || velocity.is_a?(Float)) && velocity >= 0 && velocity <= 1
            raise "hit velocity must be between 0 and 1"
        end
    end

    def validate_hit_duration(duration_beats)
        unless duration_beats.nil? ||
               ((duration_beats.is_a?(Integer) || duration_beats.is_a?(Float)) && duration_beats > 0)
            raise "hit duration_beats must be positive"
        end
    end

    def hit_timing(position)
        if !@bar_grid.nil?
            validate_grid_position(position)
            return [(position - 1) * 4.0 / @bar_grid, 4.0 / @bar_grid]
        end
        validate_beat_position(position)
        [position - 1, 0.25]
    end

    def validate_grid_position(position)
        unless position.is_a?(Integer) && position >= 1 && position <= @bar_grid
            raise "hit must be an integer step within the bar grid"
        end
    end

    def validate_beat_position(position)
        unless (position.is_a?(Integer) || position.is_a?(Float)) &&
               position >= 1 && position <= 4 && ((position - 1) * 4).to_i == (position - 1) * 4
            raise "hit must be a 16th-note beat position from 1 to 4"
        end
    end

    def validate(sample_names, return_names = [])
        validate_setup
        validate_references(sample_names, return_names)
        validate_hits
        ScoreWriter.new.validate_track_name(@name)
    end

    def validate_setup
        raise "track needs a loop" unless @bars
        raise "track needs use_sample or use_synth" unless @sample_name || @synth_name
        raise "track cannot use both sample and synth" if @sample_name && @synth_name
        raise "track needs a bar" if @bar_hits.empty?
        raise "bar count must divide four" unless 4 % @bar_hits.length == 0
    end

    def validate_references(sample_names, return_names)
        if @sample_name && !sample_names.include?(@sample_name)
            raise "undeclared sample: #{@sample_name}"
        end
        if @aux_send && !return_names.include?(@aux_send[0])
            raise "undeclared aux return: #{@aux_send[0]}"
        end
    end

    def validate_hits
        @bar_hits.each do |bar_hits|
            bar_hits.each do |hit|
                validate_hit(hit)
            end
        end
    end

    def validate_hit(hit)
        raise "sample hits cannot specify note" if @sample_name && hit[1]
        raise "synth hits need note" if @synth_name && !hit[1]
    end

    def emit(writer)
        @bars.times do |bar_index|
            @bar_hits[bar_index % @bar_hits.length].each do |hit|
                emit_hit(writer, bar_index, hit)
            end
        end
    end

    def emit_hit(writer, bar_index, hit)
        beat = bar_index * 4 + hit[0]
        if @sample_name
            writer.track_sample(@name, @sample_name, beat, hit[3])
        else
            writer.track_note(@name, hit[1], beat, hit[2], hit[3])
        end
    end

    def emit_channel(writer)
        writer.channel(@name, @ha_gain_db, @pan, @fader_db, @compressor, @bell, @aux_send) if @channel_stage > 0
    end
end

class AuxReturnPattern
    attr_reader :name

    def initialize(name)
        @name = name
        @reverb = nil
        @fader_db = nil
    end

    def reverb(room:, damp:, wet:)
        raise "aux return has more than one reverb" if @reverb
        unless room >= 0 && room <= 1 && damp >= 0 && damp <= 1 && wet >= 0 && wet <= 1
            raise "invalid reverb controls"
        end
        @reverb = [room, damp, wet]
    end

    def fader(db:)
        raise "aux return fader follows reverb" unless @reverb
        raise "aux return has more than one fader" unless @fader_db.nil?
        unless db >= -96 && db <= 24
            raise "aux return fader must be -96..24"
        end
        @fader_db = db
    end

    def validate
        ScoreWriter.new.validate_track_name(@name)
        raise "aux return needs reverb and fader" unless @reverb && !@fader_db.nil?
    end

    def emit(writer)
        writer.aux_return(@name, @reverb, @fader_db)
    end
end
