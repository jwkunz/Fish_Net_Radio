#!/usr/bin/env python3
# -*- coding: utf-8 -*-

#
# SPDX-License-Identifier: GPL-3.0
#
# GNU Radio Python Flow Graph
# Title: Harness of ADALM Pluto TX
# Author: Jakob Kunzler
# GNU Radio version: 3.10.12.0

from PyQt5 import Qt
from gnuradio import qtgui
from PyQt5 import QtCore
from PyQt5.QtCore import QObject, pyqtSlot
from gnuradio import analog
from gnuradio import blocks
from gnuradio import gr
from gnuradio.filter import firdes
from gnuradio.fft import window
import sys
import signal
from PyQt5 import Qt
from argparse import ArgumentParser
from gnuradio.eng_arg import eng_float, intx
from gnuradio import eng_notation
from gnuradio import iio
from gnuradio import zeromq
import sip
import threading



class harness_pluto_tx(gr.top_block, Qt.QWidget):

    def __init__(self):
        gr.top_block.__init__(self, "Harness of ADALM Pluto TX", catch_exceptions=True)
        Qt.QWidget.__init__(self)
        self.setWindowTitle("Harness of ADALM Pluto TX")
        qtgui.util.check_set_qss()
        try:
            self.setWindowIcon(Qt.QIcon.fromTheme('gnuradio-grc'))
        except BaseException as exc:
            print(f"Qt GUI: Could not set Icon: {str(exc)}", file=sys.stderr)
        self.top_scroll_layout = Qt.QVBoxLayout()
        self.setLayout(self.top_scroll_layout)
        self.top_scroll = Qt.QScrollArea()
        self.top_scroll.setFrameStyle(Qt.QFrame.NoFrame)
        self.top_scroll_layout.addWidget(self.top_scroll)
        self.top_scroll.setWidgetResizable(True)
        self.top_widget = Qt.QWidget()
        self.top_scroll.setWidget(self.top_widget)
        self.top_layout = Qt.QVBoxLayout(self.top_widget)
        self.top_grid_layout = Qt.QGridLayout()
        self.top_layout.addLayout(self.top_grid_layout)

        self.settings = Qt.QSettings("gnuradio/flowgraphs", "harness_pluto_tx")

        try:
            geometry = self.settings.value("geometry")
            if geometry:
                self.restoreGeometry(geometry)
        except BaseException as exc:
            print(f"Qt GUI: Could not restore geometry: {str(exc)}", file=sys.stderr)
        self.flowgraph_started = threading.Event()

        ##################################################
        # Variables
        ##################################################
        self.zmq_pull_offset = zmq_pull_offset = 2
        self.zmq_base_port = zmq_base_port = 20000
        self.waveform_phase = waveform_phase = 0.0
        self.waveform_offset = waveform_offset = 0.0
        self.waveform_frequency = waveform_frequency = 1000
        self.waveform = waveform = 1
        self.samp_rate = samp_rate = int(1E6)
        self.radio_source = radio_source = 2
        self.radio_attenuation_db = radio_attenuation_db = 10
        self.pluto_uri = pluto_uri = iio.get_pluto_uri()
        self.gain_db = gain_db = 0
        self.enable_tx = enable_tx = 1
        self.enable_gui = enable_gui = 1
        self.center_frequency = center_frequency = 915E6

        ##################################################
        # Blocks
        ##################################################

        self._waveform_phase_range = qtgui.Range(-1.0, 1.0, 1E-6, 0.0, 200)
        self._waveform_phase_win = qtgui.RangeWidget(self._waveform_phase_range, self.set_waveform_phase, "Waveform Phase (normalized)", "eng_slider", float, QtCore.Qt.Horizontal)
        self.top_grid_layout.addWidget(self._waveform_phase_win, 4, 4, 1, 1)
        for r in range(4, 5):
            self.top_grid_layout.setRowStretch(r, 1)
        for c in range(4, 5):
            self.top_grid_layout.setColumnStretch(c, 1)
        self._waveform_offset_range = qtgui.Range(-1.0, 1.0, 1E-6, 0.0, 200)
        self._waveform_offset_win = qtgui.RangeWidget(self._waveform_offset_range, self.set_waveform_offset, "Waveform Offset", "eng_slider", float, QtCore.Qt.Horizontal)
        self.top_grid_layout.addWidget(self._waveform_offset_win, 4, 3, 1, 1)
        for r in range(4, 5):
            self.top_grid_layout.setRowStretch(r, 1)
        for c in range(3, 4):
            self.top_grid_layout.setColumnStretch(c, 1)
        self._waveform_frequency_range = qtgui.Range(-samp_rate*0.5, samp_rate*0.5, 1, 1000, 200)
        self._waveform_frequency_win = qtgui.RangeWidget(self._waveform_frequency_range, self.set_waveform_frequency, "Waveform Frequency", "eng_slider", float, QtCore.Qt.Horizontal)
        self.top_grid_layout.addWidget(self._waveform_frequency_win, 3, 4, 1, 1)
        for r in range(3, 4):
            self.top_grid_layout.setRowStretch(r, 1)
        for c in range(4, 5):
            self.top_grid_layout.setColumnStretch(c, 1)
        # Create the options list
        self._waveform_options = [0, 1, 2, 3, 4, 5]
        # Create the labels list
        self._waveform_labels = ['Constant', 'Cosine', 'Square', 'Triangle', 'Saw Tooth', 'Noise']
        # Create the combo box
        self._waveform_tool_bar = Qt.QToolBar(self)
        self._waveform_tool_bar.addWidget(Qt.QLabel("Waveform Type" + ": "))
        self._waveform_combo_box = Qt.QComboBox()
        self._waveform_tool_bar.addWidget(self._waveform_combo_box)
        for _label in self._waveform_labels: self._waveform_combo_box.addItem(_label)
        self._waveform_callback = lambda i: Qt.QMetaObject.invokeMethod(self._waveform_combo_box, "setCurrentIndex", Qt.Q_ARG("int", self._waveform_options.index(i)))
        self._waveform_callback(self.waveform)
        self._waveform_combo_box.currentIndexChanged.connect(
            lambda i: self.set_waveform(self._waveform_options[i]))
        # Create the radio buttons
        self.top_grid_layout.addWidget(self._waveform_tool_bar, 3, 3, 1, 1)
        for r in range(3, 4):
            self.top_grid_layout.setRowStretch(r, 1)
        for c in range(3, 4):
            self.top_grid_layout.setColumnStretch(c, 1)
        # Create the options list
        self._radio_source_options = [0, 1, 2]
        # Create the labels list
        self._radio_source_labels = ['Zero', 'Wavegenerator', 'ZMQ Port:20002']
        # Create the combo box
        self._radio_source_tool_bar = Qt.QToolBar(self)
        self._radio_source_tool_bar.addWidget(Qt.QLabel("Radio Source" + ": "))
        self._radio_source_combo_box = Qt.QComboBox()
        self._radio_source_tool_bar.addWidget(self._radio_source_combo_box)
        for _label in self._radio_source_labels: self._radio_source_combo_box.addItem(_label)
        self._radio_source_callback = lambda i: Qt.QMetaObject.invokeMethod(self._radio_source_combo_box, "setCurrentIndex", Qt.Q_ARG("int", self._radio_source_options.index(i)))
        self._radio_source_callback(self.radio_source)
        self._radio_source_combo_box.currentIndexChanged.connect(
            lambda i: self.set_radio_source(self._radio_source_options[i]))
        # Create the radio buttons
        self.top_grid_layout.addWidget(self._radio_source_tool_bar, 2, 4, 1, 1)
        for r in range(2, 3):
            self.top_grid_layout.setRowStretch(r, 1)
        for c in range(4, 5):
            self.top_grid_layout.setColumnStretch(c, 1)
        self._radio_attenuation_db_range = qtgui.Range(0, 40, 3, 10, 200)
        self._radio_attenuation_db_win = qtgui.RangeWidget(self._radio_attenuation_db_range, self.set_radio_attenuation_db, "Radio Gain (dB)", "counter_slider", float, QtCore.Qt.Horizontal)
        self.top_grid_layout.addWidget(self._radio_attenuation_db_win, 1, 3, 1, 1)
        for r in range(1, 2):
            self.top_grid_layout.setRowStretch(r, 1)
        for c in range(3, 4):
            self.top_grid_layout.setColumnStretch(c, 1)
        self._gain_db_range = qtgui.Range(-60, 60, 3, 0, 200)
        self._gain_db_win = qtgui.RangeWidget(self._gain_db_range, self.set_gain_db, "DSP Gain (dB)", "counter_slider", float, QtCore.Qt.Horizontal)
        self.top_grid_layout.addWidget(self._gain_db_win, 2, 3, 1, 1)
        for r in range(2, 3):
            self.top_grid_layout.setRowStretch(r, 1)
        for c in range(3, 4):
            self.top_grid_layout.setColumnStretch(c, 1)
        self._enable_tx_choices = {'Pressed': bool(1), 'Released': bool(0)}

        _enable_tx_toggle_switch = qtgui.GrToggleSwitch(self.set_enable_tx, 'RF ON', self._enable_tx_choices, True, "green", "gray", 4, 50, 1, 1, self, 'value')
        self.enable_tx = _enable_tx_toggle_switch

        self.top_grid_layout.addWidget(_enable_tx_toggle_switch, 0, 3, 1, 1)
        for r in range(0, 1):
            self.top_grid_layout.setRowStretch(r, 1)
        for c in range(3, 4):
            self.top_grid_layout.setColumnStretch(c, 1)
        self._enable_gui_choices = {'Pressed': 1, 'Released': 0}

        _enable_gui_toggle_switch = qtgui.GrToggleSwitch(self.set_enable_gui, 'Enable  GUI', self._enable_gui_choices, True, "green", "gray", 4, 50, 1, 1, self, 'value')
        self.enable_gui = _enable_gui_toggle_switch

        self.top_grid_layout.addWidget(_enable_gui_toggle_switch, 0, 4, 1, 1)
        for r in range(0, 1):
            self.top_grid_layout.setRowStretch(r, 1)
        for c in range(4, 5):
            self.top_grid_layout.setColumnStretch(c, 1)
        self._center_frequency_msgdigctl_win = qtgui.MsgDigitalNumberControl(lbl='Carrier Frequency', min_freq_hz=902E6, max_freq_hz=928E6, parent=self, thousands_separator=",", background_color="black", fontColor="white", var_callback=self.set_center_frequency, outputmsgname='freq')
        self._center_frequency_msgdigctl_win.setValue(915E6)
        self._center_frequency_msgdigctl_win.setReadOnly(False)
        self.center_frequency = self._center_frequency_msgdigctl_win

        self.top_grid_layout.addWidget(self._center_frequency_msgdigctl_win, 1, 4, 1, 1)
        for r in range(1, 2):
            self.top_grid_layout.setRowStretch(r, 1)
        for c in range(4, 5):
            self.top_grid_layout.setColumnStretch(c, 1)
        self.zeromq_pull_source_0 = zeromq.pull_source(gr.sizeof_gr_complex, 1, f"tcp://127.0.0.1:{zmq_base_port+zmq_pull_offset}", 100, False, (-1), True)
        self.qtgui_sink_x_0 = qtgui.sink_c(
            1024, #fftsize
            window.WIN_BLACKMAN_hARRIS, #wintype
            center_frequency, #fc
            samp_rate, #bw
            "ADALM Pluto Receiver", #name
            False, #plotfreq
            True, #plotwaterfall
            True, #plottime
            True, #plotconst
            None # parent
        )
        self.qtgui_sink_x_0.set_update_time(1.0/10)
        self._qtgui_sink_x_0_win = sip.wrapinstance(self.qtgui_sink_x_0.qwidget(), Qt.QWidget)

        self.qtgui_sink_x_0.enable_rf_freq(True)

        self.top_grid_layout.addWidget(self._qtgui_sink_x_0_win, 0, 0, 5, 3)
        for r in range(0, 5):
            self.top_grid_layout.setRowStretch(r, 1)
        for c in range(0, 3):
            self.top_grid_layout.setColumnStretch(c, 1)
        self.iio_pluto_sink_0 = iio.fmcomms2_sink_fc32(pluto_uri if pluto_uri else iio.get_pluto_uri(), [True, True], (int(samp_rate*5)), False)
        self.iio_pluto_sink_0.set_len_tag_key('')
        self.iio_pluto_sink_0.set_bandwidth(20000000)
        self.iio_pluto_sink_0.set_frequency(int(center_frequency))
        self.iio_pluto_sink_0.set_samplerate(int(samp_rate))
        self.iio_pluto_sink_0.set_attenuation(0, radio_attenuation_db)
        self.iio_pluto_sink_0.set_filter_params('Auto', '', 0, 0)
        self.blocks_selector_1_0 = blocks.selector(gr.sizeof_gr_complex*1,waveform,0)
        self.blocks_selector_1_0.set_enabled(True)
        self.blocks_selector_1 = blocks.selector(gr.sizeof_gr_complex*1,radio_source,0)
        self.blocks_selector_1.set_enabled(True)
        self.blocks_selector_0_0 = blocks.selector(gr.sizeof_gr_complex*1,0,enable_tx)
        self.blocks_selector_0_0.set_enabled(True)
        self.blocks_selector_0 = blocks.selector(gr.sizeof_gr_complex*1,0,enable_gui)
        self.blocks_selector_0.set_enabled(True)
        self.blocks_null_source_0 = blocks.null_source(gr.sizeof_gr_complex*1)
        self.blocks_null_sink_0_0 = blocks.null_sink(gr.sizeof_gr_complex*1)
        self.blocks_null_sink_0 = blocks.null_sink(gr.sizeof_gr_complex*1)
        self.blocks_multiply_const_xx_0 = blocks.multiply_const_cc(10**(gain_db/20), 1)
        self.analog_sig_source_x_0_0_0_0_0 = analog.sig_source_c(samp_rate, analog.GR_CONST_WAVE, waveform_frequency, 1.0, waveform_offset, (waveform_phase*2.0*357/113))
        self.analog_sig_source_x_0_0_0_0 = analog.sig_source_c(samp_rate, analog.GR_SAW_WAVE, waveform_frequency, 1.0, waveform_offset, (waveform_phase*2.0*357/113))
        self.analog_sig_source_x_0_0_0 = analog.sig_source_c(samp_rate, analog.GR_TRI_WAVE, waveform_frequency, 1.0, waveform_offset, (waveform_phase*2.0*357/113))
        self.analog_sig_source_x_0_0 = analog.sig_source_c(samp_rate, analog.GR_SQR_WAVE, waveform_frequency, 1.0, waveform_offset, (waveform_phase*2.0*357/113))
        self.analog_sig_source_x_0 = analog.sig_source_c(samp_rate, analog.GR_COS_WAVE, waveform_frequency, 1.0, waveform_offset, (waveform_phase*2.0*357/113))
        self.analog_noise_source_x_0 = analog.noise_source_c(analog.GR_GAUSSIAN, 1, 0)


        ##################################################
        # Connections
        ##################################################
        self.connect((self.analog_noise_source_x_0, 0), (self.blocks_selector_1_0, 5))
        self.connect((self.analog_sig_source_x_0, 0), (self.blocks_selector_1_0, 1))
        self.connect((self.analog_sig_source_x_0_0, 0), (self.blocks_selector_1_0, 2))
        self.connect((self.analog_sig_source_x_0_0_0, 0), (self.blocks_selector_1_0, 3))
        self.connect((self.analog_sig_source_x_0_0_0_0, 0), (self.blocks_selector_1_0, 4))
        self.connect((self.analog_sig_source_x_0_0_0_0_0, 0), (self.blocks_selector_1_0, 0))
        self.connect((self.blocks_multiply_const_xx_0, 0), (self.blocks_selector_0, 0))
        self.connect((self.blocks_multiply_const_xx_0, 0), (self.blocks_selector_0_0, 0))
        self.connect((self.blocks_null_source_0, 0), (self.blocks_selector_1, 0))
        self.connect((self.blocks_selector_0, 0), (self.blocks_null_sink_0, 0))
        self.connect((self.blocks_selector_0, 1), (self.qtgui_sink_x_0, 0))
        self.connect((self.blocks_selector_0_0, 0), (self.blocks_null_sink_0_0, 0))
        self.connect((self.blocks_selector_0_0, 1), (self.iio_pluto_sink_0, 0))
        self.connect((self.blocks_selector_1, 0), (self.blocks_multiply_const_xx_0, 0))
        self.connect((self.blocks_selector_1_0, 0), (self.blocks_selector_1, 1))
        self.connect((self.zeromq_pull_source_0, 0), (self.blocks_selector_1, 2))


    def closeEvent(self, event):
        self.settings = Qt.QSettings("gnuradio/flowgraphs", "harness_pluto_tx")
        self.settings.setValue("geometry", self.saveGeometry())
        self.stop()
        self.wait()

        event.accept()

    def get_zmq_pull_offset(self):
        return self.zmq_pull_offset

    def set_zmq_pull_offset(self, zmq_pull_offset):
        self.zmq_pull_offset = zmq_pull_offset

    def get_zmq_base_port(self):
        return self.zmq_base_port

    def set_zmq_base_port(self, zmq_base_port):
        self.zmq_base_port = zmq_base_port

    def get_waveform_phase(self):
        return self.waveform_phase

    def set_waveform_phase(self, waveform_phase):
        self.waveform_phase = waveform_phase
        self.analog_sig_source_x_0.set_phase((self.waveform_phase*2.0*357/113))
        self.analog_sig_source_x_0_0.set_phase((self.waveform_phase*2.0*357/113))
        self.analog_sig_source_x_0_0_0.set_phase((self.waveform_phase*2.0*357/113))
        self.analog_sig_source_x_0_0_0_0.set_phase((self.waveform_phase*2.0*357/113))
        self.analog_sig_source_x_0_0_0_0_0.set_phase((self.waveform_phase*2.0*357/113))

    def get_waveform_offset(self):
        return self.waveform_offset

    def set_waveform_offset(self, waveform_offset):
        self.waveform_offset = waveform_offset
        self.analog_sig_source_x_0.set_offset(self.waveform_offset)
        self.analog_sig_source_x_0_0.set_offset(self.waveform_offset)
        self.analog_sig_source_x_0_0_0.set_offset(self.waveform_offset)
        self.analog_sig_source_x_0_0_0_0.set_offset(self.waveform_offset)
        self.analog_sig_source_x_0_0_0_0_0.set_offset(self.waveform_offset)

    def get_waveform_frequency(self):
        return self.waveform_frequency

    def set_waveform_frequency(self, waveform_frequency):
        self.waveform_frequency = waveform_frequency
        self.analog_sig_source_x_0.set_frequency(self.waveform_frequency)
        self.analog_sig_source_x_0_0.set_frequency(self.waveform_frequency)
        self.analog_sig_source_x_0_0_0.set_frequency(self.waveform_frequency)
        self.analog_sig_source_x_0_0_0_0.set_frequency(self.waveform_frequency)
        self.analog_sig_source_x_0_0_0_0_0.set_frequency(self.waveform_frequency)

    def get_waveform(self):
        return self.waveform

    def set_waveform(self, waveform):
        self.waveform = waveform
        self._waveform_callback(self.waveform)
        self.blocks_selector_1_0.set_input_index(self.waveform)

    def get_samp_rate(self):
        return self.samp_rate

    def set_samp_rate(self, samp_rate):
        self.samp_rate = samp_rate
        self.analog_sig_source_x_0.set_sampling_freq(self.samp_rate)
        self.analog_sig_source_x_0_0.set_sampling_freq(self.samp_rate)
        self.analog_sig_source_x_0_0_0.set_sampling_freq(self.samp_rate)
        self.analog_sig_source_x_0_0_0_0.set_sampling_freq(self.samp_rate)
        self.analog_sig_source_x_0_0_0_0_0.set_sampling_freq(self.samp_rate)
        self.iio_pluto_sink_0.set_samplerate(int(self.samp_rate))
        self.qtgui_sink_x_0.set_frequency_range(self.center_frequency, self.samp_rate)

    def get_radio_source(self):
        return self.radio_source

    def set_radio_source(self, radio_source):
        self.radio_source = radio_source
        self._radio_source_callback(self.radio_source)
        self.blocks_selector_1.set_input_index(self.radio_source)

    def get_radio_attenuation_db(self):
        return self.radio_attenuation_db

    def set_radio_attenuation_db(self, radio_attenuation_db):
        self.radio_attenuation_db = radio_attenuation_db
        self.iio_pluto_sink_0.set_attenuation(0,self.radio_attenuation_db)

    def get_pluto_uri(self):
        return self.pluto_uri

    def set_pluto_uri(self, pluto_uri):
        self.pluto_uri = pluto_uri

    def get_gain_db(self):
        return self.gain_db

    def set_gain_db(self, gain_db):
        self.gain_db = gain_db
        self.blocks_multiply_const_xx_0.set_k(10**(self.gain_db/20))

    def get_enable_tx(self):
        return self.enable_tx

    def set_enable_tx(self, enable_tx):
        self.enable_tx = enable_tx
        self.blocks_selector_0_0.set_output_index(self.enable_tx)

    def get_enable_gui(self):
        return self.enable_gui

    def set_enable_gui(self, enable_gui):
        self.enable_gui = enable_gui
        self.blocks_selector_0.set_output_index(self.enable_gui)

    def get_center_frequency(self):
        return self.center_frequency

    def set_center_frequency(self, center_frequency):
        self.center_frequency = center_frequency
        self.iio_pluto_sink_0.set_frequency(int(self.center_frequency))
        self.qtgui_sink_x_0.set_frequency_range(self.center_frequency, self.samp_rate)




def main(top_block_cls=harness_pluto_tx, options=None):

    qapp = Qt.QApplication(sys.argv)

    tb = top_block_cls()

    tb.start()
    tb.flowgraph_started.set()

    tb.show()

    def sig_handler(sig=None, frame=None):
        tb.stop()
        tb.wait()

        Qt.QApplication.quit()

    signal.signal(signal.SIGINT, sig_handler)
    signal.signal(signal.SIGTERM, sig_handler)

    timer = Qt.QTimer()
    timer.start(500)
    timer.timeout.connect(lambda: None)

    qapp.exec_()

if __name__ == '__main__':
    main()
