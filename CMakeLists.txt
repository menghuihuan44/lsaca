cmake_minimum_required(VERSION 3.20)
project(lsaca VERSION 0.0.1)

set(CMAKE_C_STANDARD 17)
set(CMAKE_C_STANDARD_REQUIRED ON)
set(CMAKE_C_EXTENSIONS OFF)

if(CMAKE_C_COMPILER_ID STREQUAL "GCC")
    list(APPEND
        -Wall
        -Wextra
        -Wpedantic
        -Wshadow
        -Wstrict-prototypes
        -Wmissing-prototypes
        -Wold-style-definition
        -Wmissing-declarations
        -Wredundant-decls
        -Wnested-externs
        -Wpointer-arith
        -Wcast-align
        -Wwrite-strings
        -Wformat=2
        -Wformat-overflow=2
        -Wformat-truncation=2
        -Wimplicit-fallthrough
        -Wmissing-include-dirs
        -Wswitch-default
        -Wuninitialized
        -Wmaybe-uninitialized
        -Wstrict-overflow=4
        -Wduplicated-branches
        -Wduplicated-cond
        -Wnull-dereference
        -Wjump-misses-init
        -Wlogical-op
        -Wno-unused-parameter
        -Wno-missing-field-initializers
    )
elseif(CMAKE_C_COMPILER_ID STREQUAL "Clang")
    list(APPEND
        -Wall
        -Wextra
        -Wpedantic
        -Wshadow
        -Wstrict-prototypes
        -Wmissing-prototypes
        -Wold-style-definition
        -Wmissing-declarations
        -Wredundant-decls
        -Wpointer-arith
        -Wcast-align
        -Wwrite-strings
        -Wformat=2
        -Wimplicit-fallthrough
        -Wswitch-default
        -Wuninitialized
        -Wnull-dereference
        -Wno-unused-parameter
        -Wno-missing-field-initializers
        -Wno-newline-eof
        -Wno-format-nonliteral
    )
elseif(CMAKE_C_COMPILER_ID STREQUAL "MSVC")
    list(APPEND
        /W4
        /wd4200
        /wd4201
        /wd4204
        /wd4221
        /wd4996
    )
endif()

if(CMAKE_BUILD_TYPE STREQUAL "Debug")
    if(CMAKE_C_COMPILER_ID STREQUAL "GCC")
        list(APPEND -O0 -g3 -ggdb -fno-omit-frame-pointer)
    elseif(CMAKE_C_COMPILER_ID STREQUAL "Clang")
        list(APPEND -O0 -g3 -fno-omit-frame-pointer)
    elseif(CMAKE_C_COMPILER_ID STREQUAL "MSVC")
        list(APPEND /Od /Zi)
    endif()
elseif(CMAKE_BUILD_TYPE STREQUAL "Release")
    if(CMAKE_C_COMPILER_ID STREQUAL "GCC")
        list(APPEND
            -O3
            -DNDEBUG
            -march=native
            -mtune=native
            -flto
            -fomit-frame-pointer
            -fstrict-aliasing
            -funroll-loops
            -finline-functions
            -ftree-vectorize
            -fvisibility=hidden
            -fdata-sections
            -ffunction-sections
        )
        set(CMAKE_EXE_LINKER_FLAGS "${CMAKE_EXE_LINKER_FLAGS} -flto -Wl")
        set(CMAKE_SHARED_LINKER_FLAGS "${CMAKE_SHARED_LINKER_FLAGS} -flto -Wl") 
    elseif(CMAKE_C_COMPILER_ID STREQUAL "Clang")
        list(APPEND
            -O3
            -DNDEBUG
            -march=native
            -mtune=native
            -flto=thin
            -fomit-frame-pointer
            -fstrict-aliasing
            -funroll-loops
            -fvisibility=hidden
            -fdata-sections
            -ffunction-sections
        )
        set(CMAKE_EXE_LINKER_FLAGS "${CMAKE_EXE_LINKER_FLAGS} -flto=thin -Wl")
        set(CMAKE_SHARED_LINKER_FLAGS "${CMAKE_SHARED_LINKER_FLAGS} -flto=thin -Wl") 
    elseif(CMAKE_C_COMPILER_ID STREQUAL "MSVC")
        list(APPEND /O2 /GL /GS- /Gy)
        list(APPEND CMAKE_EXE_LINKER_FLAGS /LTCG /OPT:REF)
        list(APPEND CMAKE_SHARED_LINKER_FLAGS /LTCG /OPT:REF)
    endif()
endif()


file(GLOB_RECURSE SOURCES "src/*.c")
file(GLOB_RECURSE HEADERS "src/*.h")

add_executable(${PROJECT_NAME} ${SOURCES} ${HEADERS})
target_include_directories(${PROJECT_NAME} PRIVATE ${CMAKE_CURRENT_SOURCE_DIR})