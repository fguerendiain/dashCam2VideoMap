#!/bin/sh

help()
{
    echo
    echo "70mai Dash Cam Lite 2 timelapse and hud map video builder tool"
    echo "--- Video to timed PNG sequence, WAV audio and GPS coordinates"
    echo
    echo "Syntax: $0 [option] [param] [option] [param] ..."
    echo
    echo "Options:"
    echo
    echo "      -h        Shows this help screen"
    echo
    echo "      -i FILE   Input video file (required)"
    echo
    echo "      -o DIR    Output dir for PNG image sequence (required)"
    echo
    echo "      -t FACTOR Time factor multiplier for timelapse (Must be greater than 0 and less or"
    echo "                equal than 1) (defaults to 1, no timelapse)"
    echo
}

INPUT_FILE=""
OUTPUT_DIR=""
TIMELAPSE_FACTOR=1

while getopts "hi:o:t:g:" option; do
    case $option in
        h)
            help
            exit;;
        i)
            INPUT_FILE=$OPTARG
            ;;
        o)
            OUTPUT_DIR=$OPTARG
            ;;
        t)
            TIMELAPSE_FACTOR=$OPTARG
            ;;
        \?)
            help
            exit;;
   esac
done

if [ "$INPUT_FILE" = "" ]; then
    echo "Input file is required"
    exit
fi

if [ "$OUTPUT_DIR" = "" ]; then
    echo "Output dir is required"
    exit
fi

ffmpeg -i $INPUT_FILE -vf "setpts=$TIMELAPSE_FACTOR*PTS" $OUTPUT_DIR/%06d.png