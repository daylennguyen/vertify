complete -c vertify -l completions -d 'Generate shell completions and exit (bash, zsh, fish, powershell, elvish)' -r -f -a "{bash	'',elvish	'',fish	'',powershell	'',zsh	''}"
complete -c vertify -s t -l to -d 'Target aspect ratio (auto = flip the input\'s orientation)' -r -f -a "{auto	'',9:16	'',16:9	''}"
complete -c vertify -s f -l fill -d 'How to fill the empty space around the video' -r -f -a "{blur	'',color	''}"
complete -c vertify -l size -d 'Length of the output\'s long edge in pixels (1920 = 1080p-class)' -r
complete -c vertify -l color -d 'Solid fill color (only used with --fill color), e.g. black, white, #101010' -r
complete -c vertify -l blur -d 'Blur strength (only used with --fill blur)' -r
complete -c vertify -l crf -d 'x264 CRF quality (lower = better, 18-28 is sane)' -r
complete -c vertify -l fast -d 'Encode as fast as possible (larger file, lower quality-per-bit)'
complete -c vertify -s y -l overwrite -d 'Overwrite the output file if it exists'
complete -c vertify -l dry-run -d 'Print the ffmpeg command instead of running it'
complete -c vertify -s h -l help -d 'Print help (see more with \'--help\')'
complete -c vertify -s V -l version -d 'Print version'
