_vertify() {
    local i cur prev opts cmd
    COMPREPLY=()
    cur="${COMP_WORDS[COMP_CWORD]}"
    prev="${COMP_WORDS[COMP_CWORD-1]}"
    cmd=""
    opts=""

    for i in ${COMP_WORDS[@]}
    do
        case "${cmd},${i}" in
            ",$1")
                cmd="vertify"
                ;;
            *)
                ;;
        esac
    done

    case "${cmd}" in
        vertify)
            opts="-t -f -y -h -V --output-dir --suffix --completions --to --fill --size --color --blur --fast --preset --crf --overwrite --dry-run --ffmpeg-arg --audio-mode --audio-bitrate --map-metadata --start --duration --json-plan --loglevel --no-faststart --open --help --version [INPUT] [OUTPUT]"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 1 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                --output-dir)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --suffix)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --completions)
                    COMPREPLY=($(compgen -W "bash elvish fish powershell zsh" -- "${cur}"))
                    return 0
                    ;;
                --to)
                    COMPREPLY=($(compgen -W "auto 9:16 16:9" -- "${cur}"))
                    return 0
                    ;;
                -t)
                    COMPREPLY=($(compgen -W "auto 9:16 16:9" -- "${cur}"))
                    return 0
                    ;;
                --fill)
                    COMPREPLY=($(compgen -W "blur color" -- "${cur}"))
                    return 0
                    ;;
                -f)
                    COMPREPLY=($(compgen -W "blur color" -- "${cur}"))
                    return 0
                    ;;
                --size)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --color)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --blur)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --preset)
                    COMPREPLY=($(compgen -W "ultrafast superfast veryfast faster fast medium slow slower veryslow placebo" -- "${cur}"))
                    return 0
                    ;;
                --crf)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --ffmpeg-arg)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --audio-mode)
                    COMPREPLY=($(compgen -W "copy aac none" -- "${cur}"))
                    return 0
                    ;;
                --audio-bitrate)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --start)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --duration)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --loglevel)
                    COMPREPLY=($(compgen -W "quiet error warning info" -- "${cur}"))
                    return 0
                    ;;
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
    esac
}

if [[ "${BASH_VERSINFO[0]}" -eq 4 && "${BASH_VERSINFO[1]}" -ge 4 || "${BASH_VERSINFO[0]}" -gt 4 ]]; then
    complete -F _vertify -o nosort -o bashdefault -o default vertify
else
    complete -F _vertify -o bashdefault -o default vertify
fi
