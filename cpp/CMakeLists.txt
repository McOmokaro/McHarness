cmake_minimum_required(VERSION 3.20)
project(mcharness_speed LANGUAGES CXX)

set(CMAKE_CXX_STANDARD 20)
set(CMAKE_CXX_STANDARD_REQUIRED ON)

add_library(mcharness_speed SHARED
    src/stream.cpp
    src/memsearch.cpp
    src/embed.cpp
)

target_include_directories(mcharness_speed PUBLIC include)

if(NOT MSVC)
    target_compile_options(mcharness_speed PRIVATE -O3 -march=native)
endif()
