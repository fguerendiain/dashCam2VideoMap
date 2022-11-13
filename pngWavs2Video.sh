#!/bin/sh

help()
{
    echo
    echo "70mai Dash Cam Lite 2 timelapse and hud map video builder tool"
    echo "--- PNG + Audio to encoded video"
    echo
    echo "Syntax: $0 [option] [param] [option] [param] ..."
    echo
    echo "Options:"
    echo
    echo "      -h        Shows this help screen"
    echo
    echo "      -i DIR   Input dir for PNG image sequence (required)"
    echo
    echo "      -o FILE  Output video file (required)"
    echo
}

INPUT_DIR=""
OUTPUT_FILE=""

while getopts "hi:o:" option; do
    case $option in
        h)
            help
            exit;;
        i)
            INPUT_DIR=$OPTARG
            ;;
        o)
            OUTPUT_FILE=$OPTARG
            ;;
        \?)
            help
            exit;;
   esac
done

if [ "$INPUT_DIR" = "" ]; then
    echo "Input dir is required"
    exit
fi

if [ "$OUTPUT_FILE" = "" ]; then
    echo "Output file is required"
    exit
fi

ffmpeg -framerate 30 -i $INPUT_DIR/%06d.png -c:v libx265 -preset fast $OUTPUT_FILE/%06d.mkv