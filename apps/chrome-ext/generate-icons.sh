#!/bin/bash

# Generate placeholder icons for Chrome extension
# Creates simple colored squares with "ML" text

mkdir -p icons

# Function to create an icon with ImageMagick (if available) or fallback
create_icon() {
  size=$1
  output="icons/icon-${size}.png"

  if command -v convert &> /dev/null; then
    # Use ImageMagick if available
    convert -size ${size}x${size} xc:'#3b82f6' \
            -gravity center -fill white \
            -font Helvetica-Bold -pointsize $((size/3)) \
            -annotate +0+0 'ML' \
            "$output"
  elif command -v sips &> /dev/null; then
    # macOS fallback using sips - create a simple colored square
    # Create a simple colored PNG using Python
    python3 -c "
from PIL import Image, ImageDraw, ImageFont
import sys
size = $size
img = Image.new('RGB', (size, size), color='#3b82f6')
img.save('$output')
" 2>/dev/null || {
      # Final fallback: create a 1x1 PNG and resize
      echo "Creating simple placeholder icon for size $size"
      # Create a base64-encoded 1x1 blue pixel PNG
      echo "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNkZ/j/HwAC/wH/CNd6hQAAAABJRU5ErkJggg==" | base64 -d > "$output"
    }
  else
    # Linux fallback - create with base64
    echo "Creating placeholder icon $size x $size"
    echo "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNkZ/j/HwAC/wH/CNd6hQAAAABJRU5ErkJggg==" | base64 -d > "$output"
  fi

  echo "Created $output"
}

# Generate icons in required sizes
create_icon 16
create_icon 48
create_icon 128

echo "Icons generated successfully!"