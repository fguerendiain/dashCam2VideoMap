#!/bin/sh

VIDEO2TIMES_PNG_WAV="./video2timedPngWavGPS.sh"

help()
{
    echo
    echo "70mai Dash Cam Lite 2 timelapse and hud map video builder tool"
    echo
    echo "Syntax: $0 [option] [param] [option] [param] ..."
    echo
    echo "Options:"
    echo
    echo "      -h        Shows this help screen"
    echo
    echo "      -i DIR    Input directory containing the root directory of the SD written by the"
    echo "                dashcam (defaults to current directory)"
    echo
    echo "      -w DIR    Working directory for temporary files (defaults to /tmp/dashcam2VideoMap/)"
    echo
    echo "      -a DIR    Audio directory from where to read supported audio files to be used in the"
    echo "                final video (optional, if none, no audio is added)"
    echo
    echo "      -s        Use original video audio. It's mixed with the audio provided in -a if present"
    echo
    echo "      -o FILE   Output video file (Extension defines the container, defaults to"
    echo "                mkv) (defaults to out.mkv)"
    echo
    echo "      -t FACTOR Time factor multiplier for timelapse (Must be greater than 0 and less or"
    echo "                equal than 1) (defaults to 1, no timelapse)"
    echo
    echo "      -y        Unatended mode"
    echo
}

buildOutputFile()
{
    BASE_NAME=${1%.*}
    EXTENSION=${1#$BASE_NAME.}

    if [ $EXTENSION = $BASE_NAME ]; then
        EXTENSION=""
    fi

    if [ "$EXTENSION" ]; then
        OUTPUT_FILE=$1
    else
        OUTPUT_FILE=$BASE_NAME.mkv
    fi
}

processVideo()
{
    FILE_NAME=$(basename ${1})
    BASE_NAME=${FILE_NAME%.*}
    OUTPUT_DIR=$TMP_DIR/$2/$BASE_NAME/
    mkdir -p $OUTPUT_DIR
    $VIDEO2TIMES_PNG_WAV -i $FILE -o $OUTPUT_DIR -t $TIMELAPSE_FACTOR -g $3
}

INPUT_DIR="./"
TMP_DIR="/tmp/dashcam2VideoMap"
AUDIO_DIR=""
ORIGINAL_SOUND=false
OUTPUT_FILE="./out.mkv"
TIMELAPSE_FACTOR=1
UNATENDED=false

while getopts "hi:w:a:so:t:y" option; do
    case $option in
        h)
            help
            exit;;
        i)
            INPUT_DIR=$OPTARG
            ;;
        w)
            TMP_DIR=$OPTARG
            ;;
        a)
            AUDIO_DIR=$OPTARG
            ;;
        s)
            ORIGINAL_SOUND=true
            ;;
        o)
            buildOutputFile $OPTARG
            ;;
        t)
            TIMELAPSE_FACTOR=$OPTARG
            ;;
        y)
            UNATENDED=true
            ;;
        \?)
            help
            exit;;
   esac
done

echo "Summary"
echo "~~~~~~~"
echo "Input:    $INPUT_DIR"
echo "Tmp:      $TMP_DIR"
echo "Audio:    $AUDIO_DIR"
echo "Sound:    $ORIGINAL_SOUND"
echo "Time:     $TIMELAPSE_FACTOR"
echo "Output:   $OUTPUT_FILE"
echo ""

if [ $UNATENDED = false ]; then
    read -n 1 -p "Start the process: [Y/n]: " ANSWER
    if [ "$ANSWER" = "n" ]; then
        exit
    fi
fi

echo "Starting the show!!"

GPS_FILE=$TMP_DIR/gpsData.txt

cat $INPUT_DIR/Normal/GPSData*.txt > $GPS_FILE

echo "Processing front camera"
for FILE in $INPUT_DIR/Normal/Front/* ; do
    processVideo $FILE "Front" $GPS_FILE
done

echo "Processing back camera"
for FILE in $INPUT_DIR/Normal/Back/* ; do
   processVideo $FILE "Back" $GPS_FILE
done


# - Por cada directorio generado previamente ejecuta dash2Map con los datos pertinentes y va guardando el resultado en un directorio
# - Calcula la duración del video
# - Con eso calcula cuánto audio hay que usar
# - Calcula la cantidad de cuadros por segundo en base a la cantidad de cuadros por segundo del video original, la cantidad de cuadros por segundo especificados y el multiplicador de tiempo especificado
# - Llama a pngWavs2video.sh con los valores correspondientes
# - Borra todo lo que hay en el directorio de trabajo (salvo que por parámetro se aclare que no)




