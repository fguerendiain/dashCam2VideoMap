#!/bin/sh

# Eventualmente la idea es que este otro sh esté "instalado" como parte del paquete
VIDEO2TIME_PNG="./video2timedPng.sh"
DASH_TO_MAP="./dash_to_map"

# No termino de entender el tema de los husos horarios en los archivos así que de momento los hardcodeo
VIDEOFILES_TIMEZONE=-3
GPSDATA_TIMEZONE=-8
OUTPUT_TIMEZONE=-3

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
    echo "      -p WIDTH  Width of the output video file (defaults to 1920)"
    echo
    echo "      -y        Unatended mode"
    echo
    echo "      -m DIR    Map cahce dir (defaults to ~/.cache/dashmap"
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
    YEAR=${BASE_NAME:2:4}
    MONTH=${BASE_NAME:6:2}
    DAY=${BASE_NAME:8:2}
    HOUR=${BASE_NAME:11:2}
    MINUTE=${BASE_NAME:13:2}
    SECOND=${BASE_NAME:15:2}
    EPOCH=$(date -d"$YEAR/$MONTH/$DAY $HOUR:$MINUTE:$SECOND" "+%s")
    OUTPUT_DIR=$TMP_DIR/$2/$BASE_NAME/
    mkdir -p $OUTPUT_DIR
    $VIDEO2TIME_PNG -i $FILE -o $OUTPUT_DIR -t $3
    echo $EPOCH > $OUTPUT_DIR/metadata.txt
}

INPUT_DIR="./"
TMP_DIR="/tmp/dashcam2VideoMap"
AUDIO_DIR=""
ORIGINAL_SOUND=false
OUTPUT_FILE="./out.mkv"
TIMELAPSE_FACTOR=1
UNATENDED=false
WIDTH=1920
DASHMAP_CACHE="~/.cache/dashmap"

while getopts "hi:w:a:so:t:yp:" option; do
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
        p)
            WIDTH=$OPTARG
            ;;
        m)
            DASHMAP_CACHE=$OPTARG
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
mkdir -p $TMP_DIR
cat $INPUT_DIR/GPSData*.txt > $GPS_FILE

echo "Processing front camera"
for FILE in $INPUT_DIR/Normal/Front/* ; do
    processVideo $FILE "Front" $TIMELAPSE_FACTOR
done

echo "Processing back camera"
BACK_TIMELAPSE_FACTOR=$(echo "$TIMELAPSE_FACTOR*1.2"|bc)
for FILE in $INPUT_DIR/Normal/Back/* ; do
  processVideo $FILE "Back" $BACK_TIMELAPSE_FACTOR
done

SEQ_OUTPUT=$TMP_DIR/sequence_output/
mkdir -p $SEQ_OUTPUT

FRONT_OFFSET=0
BACK_OFFSET=0
for DIR in $TMP_DIR/Front/* ; do
    FILE_NAME=$(basename $DIR)
    BASE_NAME=${FILE_NAME%.*}
    BASE_DIR=${BASE_NAME:0:24}
    FRONT_NAME=$TMP_DIR/Front/$BASE_DIR"F"
    BACK_NAME=$TMP_DIR/Back/$BASE_DIR"B"

    echo $DASH_TO_MAP --frontdir $FRONT_NAME --backdir $BACK_NAME --frameoffset $FRONT_OFFSET --width $WIDTH --outputdir $SEQ_OUTPUT --mapcachedir $DASHMAP_CACHE --originalfps 30 --originaltimefactor $TIMELAPSE_FACTOR --gpsdatafile $GPS_FILE --frontverticaloffset 200
    
    FILE_COUNT=$(ls -1q $FRONT_NAME/*.png | wc -l)
    FRONT_OFFSET=$(($FRONT_OFFSET+$FILE_COUNT))
    
    if [ -f $BACK_NAME ]
    then
        FILE_COUNT=$(ls -1q $BACK_NAME/*.png | wc -l)
        BACK_OFFSET=$(($BACK_OFFSET+$FILE_COUNT))
    fi
done