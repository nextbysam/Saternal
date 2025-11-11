#!/bin/bash

# Test script for ANSI terminal styling features

echo "=== Testing ANSI Text Styling ==="
echo ""

echo "1. Bold text:"
echo -e "\e[1mThis is BOLD text\e[0m"
echo -e "\e[1;31mBold Red\e[0m"
echo -e "\e[1;32mBold Green\e[0m"
echo -e "\e[1;33mBold Yellow\e[0m"
echo ""

echo "2. Underlined text:"
echo -e "\e[4mThis is UNDERLINED text\e[0m"
echo -e "\e[4;32mUnderlined Green\e[0m"
echo -e "\e[4;34mUnderlined Blue\e[0m"
echo ""

echo "3. Reverse video (inverted colors):"
echo -e "\e[7mThis is REVERSE video\e[0m"
echo -e "\e[7;33mReverse Yellow\e[0m"
echo -e "\e[7;35mReverse Magenta\e[0m"
echo ""

echo "4. Combined styles:"
echo -e "\e[1;4;31mBold + Underlined Red\e[0m"
echo -e "\e[1;7;32mBold + Reverse Green\e[0m"
echo -e "\e[4;7;34mUnderlined + Reverse Blue\e[0m"
echo ""

echo "5. All 16 ANSI colors:"
for i in {0..7}; do
    echo -e "\e[3${i}mColor $i: Normal\e[0m  \e[1;3${i}mColor $i: Bold\e[0m"
done
echo ""

echo "6. Background colors with text:"
echo -e "\e[41mRed background\e[0m"
echo -e "\e[42mGreen background\e[0m"
echo -e "\e[43mYellow background\e[0m"
echo -e "\e[44mBlue background\e[0m"
echo -e "\e[45mMagenta background\e[0m"
echo -e "\e[46mCyan background\e[0m"
echo -e "\e[47mWhite background\e[0m"
echo ""

echo "7. Foreground + Background combinations:"
echo -e "\e[30;47mBlack on White\e[0m"
echo -e "\e[31;44mRed on Blue\e[0m"
echo -e "\e[32;45mGreen on Magenta\e[0m"
echo -e "\e[33;46mYellow on Cyan\e[0m"
echo ""

echo "=== Test Complete ==="
